use super::response::{ActionModResponse, ActionResponse, ConstraintResponse};
use crate::{
    SysinspectError,
    cfg::mmconf::{DEFAULT_MODULES_DIR, DEFAULT_MODULES_LIB_DIR},
    inspector::SysInspectRunner,
    intp::{
        actproc::response::{ConstraintFailure, ConstraintPass},
        constraints::{Constraint, ConstraintKind, ExprRes},
        functions,
        inspector::get_cfg_sharelib,
    },
    mdescr::{
        DSL_ACTION_CONDITION_FSZC, DSL_ACTION_CONDITION_GID, DSL_ACTION_CONDITION_UID, DSL_ACTION_CONDITION_VMEM, DSL_ACTION_CONDITION_WDIR,
        DSL_ACTION_CONDITION_WDISK,
    },
    pylang,
    util::dataconv,
};
use core::str;
use indexmap::IndexMap;
use nix::unistd::{Gid, setgid, setgroups};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_yaml::Value;
use std::{ffi::OsStr, path::Path, process::Child};
use std::{
    fmt::Display,
    io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
    vec,
};
use std::{io::Read, os::unix::process::CommandExt};

#[derive(Debug)]
pub struct SpawnSpec<'a> {
    pub module: &'a OsStr, // program path/name
    pub args: &'a [&'a str],
    pub json_in: &'a [u8], // what you write to stdin
    pub workdir: &'a str,  // your mounted quota dir
    pub uid: u32,
    pub gid: u32,
    pub fsize_cap: u64, // bytes for RLIMIT_FSIZE (0 = no cap)
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModCall {
    // Action Id
    aid: String,

    // Bind Id for an entity
    eid: String,

    // Action state
    state: String,

    // Path to the executable module
    module: PathBuf,

    // Action constraints
    constraints: Vec<Constraint>,

    // Module params
    args: IndexMap<String, Value>,
    opts: Vec<String>,
    conditions: IndexMap<String, Value>,
}

impl ModCall {
    /// Set state
    pub fn set_state(mut self, state: String) -> Self {
        self.state = state;
        self
    }

    /// Set resolved module physical path
    pub fn set_module(mut self, modpath: PathBuf) -> Self {
        self.module = modpath;
        self
    }

    /// Set unresolved module by its namespace.
    pub fn set_module_ns(self, ns: &str, sharelib: PathBuf) -> Self {
        let modpath = sharelib
            .join(DEFAULT_MODULES_DIR)
            .join(ns.trim_start_matches('.').trim_end_matches('.').trim().split('.').map(|s| s.to_string()).collect::<Vec<String>>().join("/"));
        let pymodpath = modpath.parent().unwrap().join(format!("{}.py", modpath.file_name().unwrap().to_os_string().to_str().unwrap_or_default()));
        if pymodpath.exists() {
            log::debug!("Path to a Python module: {}", pymodpath.to_str().unwrap_or_default());
            self.set_module(pymodpath)
        } else if modpath.exists() {
            log::debug!("Path to a native module: {}", modpath.to_str().unwrap_or_default());
            self.set_module(modpath)
        } else {
            log::error!("Module {} was not found in {}", ns, modpath.to_str().unwrap_or_default());
            self.set_module(PathBuf::default())
        }
    }

    /// Add a pair of kwargs
    pub fn add_kwargs(&mut self, kw: String, arg: Value) -> &mut Self {
        self.args.insert(kw, arg);
        self
    }

    /// Add an option
    pub fn add_opt(&mut self, opt: String) -> &mut Self {
        self.opts.push(opt);
        self
    }

    /// Add a condition
    pub fn add_condition(&mut self, cond: String, v: Value) -> &mut Self {
        self.conditions.insert(cond, v);
        self
    }

    /// Get a condition by its name
    pub fn get_condition(&self, cond: &str) -> Option<&Value> {
        let c = [
            DSL_ACTION_CONDITION_UID,
            DSL_ACTION_CONDITION_GID,
            DSL_ACTION_CONDITION_VMEM,
            DSL_ACTION_CONDITION_FSZC,
            DSL_ACTION_CONDITION_WDIR,
            DSL_ACTION_CONDITION_WDISK,
        ];
        if c.contains(&cond) {
            self.conditions.get(cond)
        } else {
            log::warn!("Module is requesting unknown condition: {}", cond);
            None
        }
    }

    /// Set constraints
    pub fn set_constraints(mut self, cstr: Vec<Constraint>) -> Self {
        self.constraints = cstr;

        self
    }

    /// Serialise args and opts to a JSON string for the call.
    fn params_json(&self) -> String {
        let mut out: IndexMap<String, serde_json::Value> = IndexMap::default();
        if !self.args.is_empty() {
            out.insert("arguments".to_string(), json!(self.args));
        }

        if !self.opts.is_empty() {
            out.insert("options".to_string(), json!(self.opts));
        }

        out.insert("config".to_string(), SysInspectRunner::minion_cfg_json());

        let x = json!(out).to_string();
        log::trace!("Params: {x}");
        x
    }

    /// Get module namespace.
    fn get_mod_ns(&self) -> Option<String> {
        let mut tkn = self.module.components().rev();
        let mut ns = Vec::new();

        for _ in 0..2 {
            if let Some(component) = tkn.next() {
                if let Some(part) = component.as_os_str().to_str() {
                    ns.push(part);
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        Some(ns.into_iter().rev().collect::<Vec<&str>>().join("."))
    }

    /// All expressions must be true
    fn eval_cst_all(&self, cstr: &Constraint, resp: &ActionModResponse) -> (Option<bool>, Option<Vec<String>>, Vec<ExprRes>) {
        let mut er: Vec<ExprRes> = Vec::new();
        let exp = cstr.all(self.state());
        if exp.is_empty() {
            return (None, None, er);
        }

        for exp in exp {
            let fact = functions::get_by_namespace(resp.data(), &exp.get_fact_namespace());
            let res = exp.eval(fact.to_owned());
            er.push(res.to_owned());

            // Skip infos
            if res.is_info() {
                continue;
            }

            if !res.is_positive() {
                let mut traces: Vec<String> = vec![format!(
                    "{} fact {}{}",
                    &exp.get_fact_namespace(),
                    if fact.is_none() { "data was not found" } else { "fails as " },
                    dataconv::to_string(fact).unwrap_or_default()
                )];
                traces.extend(res.traces().to_owned());
                return (Some(false), Some(traces), er);
            }
        }

        (Some(true), Some(vec![]), er)
    }

    /// At least one of the expressions must be true
    fn eval_cst_any(&self, cstr: &Constraint, resp: &ActionModResponse) -> (Option<bool>, Option<Vec<String>>, Vec<ExprRes>) {
        let mut er: Vec<ExprRes> = Vec::new();
        let exp = cstr.any(self.state());
        if exp.is_empty() {
            return (None, None, er);
        }

        let mut traces: Vec<String> = vec![];
        for exp in exp {
            let res = exp.eval(functions::get_by_namespace(resp.data(), &exp.get_fact_namespace()));
            er.push(res.to_owned());

            // Skip infos
            if res.is_info() {
                continue;
            }

            if res.is_positive() {
                return (Some(true), None, er);
            }
            traces.extend(res.traces().to_owned());
        }

        (Some(true), Some(traces), er)
    }

    /// None of expressions should be true. It is basically !all.
    fn eval_cst_none(&self, cstr: &Constraint, resp: &ActionModResponse) -> (Option<bool>, Option<Vec<String>>, Vec<ExprRes>) {
        let mut er: Vec<ExprRes> = Vec::new();
        let exp = cstr.none(self.state());
        if exp.is_empty() {
            return (None, None, er);
        }

        for e in exp {
            let fact = functions::get_by_namespace(resp.data(), &e.get_fact_namespace());
            let res = e.eval(fact.to_owned());
            er.push(res.to_owned());

            // SKip infos
            if res.is_info() {
                continue;
            }

            if res.is_positive() {
                let mut traces: Vec<String> =
                    vec![format!("{} fails with {}", &e.get_fact_namespace(), dataconv::to_string(fact).unwrap_or_default())];
                traces.extend(res.traces().to_owned());
                return (Some(false), Some(traces), er);
            }
        }

        (Some(true), None, er)
    }

    /// Evaluate constraints
    fn eval_constraints(&self, ar: &ActionModResponse) -> ConstraintResponse {
        fn eval<F>(mc: &ModCall, cret: &mut ConstraintResponse, c: &Constraint, kind: ConstraintKind, eval_fn: F, ar: &ActionModResponse)
        where
            F: Fn(&ModCall, &Constraint, &ActionModResponse) -> (Option<bool>, Option<Vec<String>>, Vec<ExprRes>),
        {
            let (res, msgs, expr) = eval_fn(mc, c, ar);
            cret.set_eval_results(expr);
            if let Some(res) = res {
                if !res {
                    cret.add_failure(ConstraintFailure::new(c.id(), c.descr(), msgs.unwrap_or(vec![]).join(" - "), kind.clone()));
                } else {
                    cret.add_pass(ConstraintPass::new(c.id()));
                }
            }
        }

        let mut cret = ConstraintResponse::new(format!("{} with {}", self.aid, self.get_mod_ns().unwrap_or("(unknown)".to_string())));
        for c in &self.constraints {
            eval(self, &mut cret, c, ConstraintKind::All, Self::eval_cst_all, ar);
            eval(self, &mut cret, c, ConstraintKind::Any, Self::eval_cst_any, ar);
            eval(self, &mut cret, c, ConstraintKind::None, Self::eval_cst_none, ar);
        }

        cret
    }

    /// Run the module
    pub fn run(&self) -> Result<Option<ActionResponse>, SysinspectError> {
        if self.module.extension().unwrap_or_default().to_str().unwrap_or_default().eq("py") {
            self.run_python_module()
        } else {
            self.run_native_module()
        }
    }

    /// Runs python script module
    fn run_python_module(&self) -> Result<Option<ActionResponse>, SysinspectError> {
        log::debug!("Calling Python module: {}", self.module.as_os_str().to_str().unwrap_or_default());

        let opts = self.opts.iter().map(|v| json!(v)).collect::<Vec<serde_json::Value>>();
        let args = self.args.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<IndexMap<String, serde_json::Value>>();

        // TODO: Add libpath and modpath here! Must come from MinionConfig
        match pylang::pvm::PyVm::new(get_cfg_sharelib().join(DEFAULT_MODULES_LIB_DIR), get_cfg_sharelib().join(DEFAULT_MODULES_DIR)).as_ptr().call(
            &self.module,
            Some(opts),
            Some(args),
        ) {
            Ok(out) => match serde_json::from_str::<ActionModResponse>(&out) {
                Ok(r) => Ok(Some(ActionResponse::new(
                    self.eid.to_owned(),
                    self.aid.to_owned(),
                    self.state.to_owned(),
                    r.clone(),
                    self.eval_constraints(&r),
                ))),
                Err(e) => Err(SysinspectError::ModuleError(format!("JSON error: {e}"))),
            },
            Err(err) => Err(err),
        }
    }

    fn to_io<E: std::fmt::Display>(e: E) -> io::Error {
        io::Error::other(e.to_string())
    }

    /// Spawn, drop to uid/gid, cap single-file size, write json to stdin, return stdout as String
    fn spawn(&self, spec: &SpawnSpec) -> io::Result<String> {
        let uid = spec.uid;
        let gid = spec.gid;
        let fsize_cap = spec.fsize_cap;

        let mut cmd = Command::new(spec.module);
        cmd.args(spec.args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

        // Change working dir, if specified
        if !spec.workdir.is_empty() && Path::new(spec.workdir).exists() {
            cmd.current_dir(spec.workdir);
        }

        log::debug!("Setting up environment and privileges: {:?}", cmd);

        unsafe {
            cmd.pre_exec(move || {
                // Harden defaults
                libc::umask(0o077);

                // Drop supplementary groups (must be before setgid/setuid)
                setgroups(&[]).map_err(Self::to_io)?;

                // Set primary GID first
                setgid(Gid::from_raw(gid)).map_err(Self::to_io)?;

                // Block priv-escalation via setuid binaries
                if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                    log::error!("Failed to set no new privs");
                    return Err(io::Error::last_os_error());
                }

                // Cap single file size if requested
                if fsize_cap > 0 {
                    let lim = libc::rlimit { rlim_cur: fsize_cap, rlim_max: fsize_cap };
                    let rc = libc::setrlimit(libc::RLIMIT_FSIZE, &lim as *const _);
                    if rc != 0 {
                        log::error!("Failed to set file size limit");
                        return Err(io::Error::last_os_error());
                    }
                }

                // Drop to UID (ruid/euid/suid are real/effective/saved respectively)
                let rc = libc::setresuid(uid, uid, uid);
                if rc != 0 {
                    log::error!("Failed to drop privileges to UID {}", uid);
                    return Err(io::Error::last_os_error());
                }

                Ok(())
            })
        };

        log::debug!("Spawning child process: {:?}", cmd);
        let mut child: Child = cmd.spawn()?;

        log::debug!("Writing JSON to stdin: {}", String::from_utf8_lossy(spec.json_in));

        if let Some(mut sin) = child.stdin.take() {
            sin.write_all(spec.json_in)?;
            // Necessary to close stdin so child can see EOF
            drop(sin);
        }

        let mut so = child.stdout.take().unwrap();
        let mut se = child.stderr.take().unwrap();

        let t_out = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = so.read_to_string(&mut s);
            s
        });
        let t_err = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = se.read_to_end(&mut b);
            b
        });

        let status = child.wait()?;
        let out = t_out.join().map_err(|_| io::Error::other("Failed to join STDOUT thread"))?;
        let err = t_err.join().map_err(|_| io::Error::other("Failed to join STDERR thread"))?;

        if !status.success() {
            return Err(io::Error::other(format!("child exit {status:?}; stderr: {}", String::from_utf8_lossy(&err))));
        }
        Ok(out)
    }

    /// Runs native external module
    fn run_native_module(&self) -> Result<Option<ActionResponse>, SysinspectError> {
        log::debug!("Calling native module: {}", self.module.as_os_str().to_str().unwrap_or_default());
        log::debug!("Params: {}", self.params_json());
        log::debug!("Opts: {:?}", self.opts);
        log::debug!("Conditions: {:?}", self.conditions);

        let muid = unsafe { libc::getuid() };
        let mgid = unsafe { libc::getgid() };

        if muid == 0 && mgid == 0 {
            let binding = self.params_json();
            // XXX: fsize-cap, working dir/disk and vmem still needs to be implemented
            let spec = SpawnSpec {
                module: self.module.as_os_str(),
                args: &[],
                json_in: binding.as_bytes(),
                workdir: self.get_condition(DSL_ACTION_CONDITION_WDIR).and_then(|v| v.as_str()).unwrap_or(""),
                uid: self.get_condition(DSL_ACTION_CONDITION_UID).and_then(|v| v.as_u64()).map(|v| v as u32).unwrap_or(muid),
                gid: self.get_condition(DSL_ACTION_CONDITION_GID).and_then(|v| v.as_u64()).map(|v| v as u32).unwrap_or(mgid),
                fsize_cap: 10 * 1024 * 1024,
            };

            log::debug!("Spawning module with spec: {:?}", spec);

            match Self::spawn(self, &spec) {
                Ok(out) => match str::from_utf8(out.as_bytes()) {
                    Ok(out) => match serde_json::from_str::<ActionModResponse>(out) {
                        Ok(r) => {
                            let mut data = r.clone();
                            data.add_data("run-uid", json!(spec.uid));
                            data.add_data("run-gid", json!(spec.gid));

                            Ok(Some(ActionResponse::new(
                                self.eid.to_owned(),
                                self.aid.to_owned(),
                                self.state.to_owned(),
                                data,
                                self.eval_constraints(&r),
                            )))
                        }
                        Err(e) => {
                            log::debug!("STDOUT: {out}");
                            Err(SysinspectError::ModuleError(format!("JSON error: {e}")))
                        }
                    },
                    Err(err) => Err(SysinspectError::ModuleError(format!("Error obtaining the output: {err}"))),
                },
                Err(err) => Err(SysinspectError::ModuleError(format!("Error calling module: {err}"))),
            }
        } else {
            log::debug!("Spawning module with default privileges");
            match Command::new(&self.module).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
                Ok(mut p) => {
                    // Send options
                    if let Some(mut stdin) = p.stdin.take()
                        && let Err(err) = stdin.write_all(self.params_json().as_bytes())
                    {
                        return Err(SysinspectError::ModuleError(format!("Error while communicating with the module: {err}")));
                    }

                    // Get the output
                    if let Ok(out) = p.wait_with_output() {
                        match str::from_utf8(&out.stdout) {
                            Ok(out) => match serde_json::from_str::<ActionModResponse>(out) {
                                Ok(r) => {
                                    let mut data = r.clone();
                                    data.add_data("run-uid", json!(muid));
                                    data.add_data("run-gid", json!(mgid));
                                    Ok(Some(ActionResponse::new(
                                        self.eid.to_owned(),
                                        self.aid.to_owned(),
                                        self.state.to_owned(),
                                        data,
                                        self.eval_constraints(&r),
                                    )))
                                }
                                Err(e) => {
                                    log::debug!("STDOUT: {out}");
                                    Err(SysinspectError::ModuleError(format!("JSON error: {e}")))
                                }
                            },
                            Err(err) => Err(SysinspectError::ModuleError(format!("Error obtaining the output: {err}"))),
                        }
                    } else {
                        Err(SysinspectError::ModuleError("Module returned no output".to_string()))
                    }
                }
                Err(err) => Err(SysinspectError::ModuleError(format!("Error calling module: {err}"))),
            }
        }
    }

    pub fn state(&self) -> String {
        self.state.to_owned()
    }

    /// Get state ref
    pub fn with_state(&self, state: String) -> bool {
        self.state == state
    }

    /// Set action Id
    pub(crate) fn set_aid(mut self, aid: String) -> Self {
        self.aid = aid;
        self
    }

    pub(crate) fn set_eid(mut self, eid: String) -> Self {
        self.eid = eid;
        self
    }
}

impl Default for ModCall {
    fn default() -> Self {
        Self {
            state: "$".to_string(),
            aid: "".to_string(),
            eid: "".to_string(),
            module: PathBuf::default(),
            args: IndexMap::default(),
            opts: Vec::default(),
            constraints: Vec::default(),
            conditions: IndexMap::default(),
        }
    }
}

impl Display for ModCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModCall - State: {}, Module: {:?}, Opts: {:?}, Args: {:?}", self.state, self.module, self.opts, self.args)?;
        Ok(())
    }
}
