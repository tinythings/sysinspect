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
    pylang,
    util::dataconv,
};
use core::str;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fmt::Display,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    vec,
};

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
    args: IndexMap<String, String>, // XXX: Should be String/Value, not String/String!
    opts: Vec<String>,
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
    pub fn add_kwargs(&mut self, kw: String, arg: String) -> &mut Self {
        self.args.insert(kw, arg);
        self
    }

    /// Add an option
    pub fn add_opt(&mut self, opt: String) -> &mut Self {
        self.opts.push(opt);
        self
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

    /// Runs native external module
    fn run_native_module(&self) -> Result<Option<ActionResponse>, SysinspectError> {
        log::debug!("Calling native module: {}", self.module.as_os_str().to_str().unwrap_or_default());
        log::debug!("Params: {}", self.params_json());

        match Command::new(&self.module).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
            Ok(mut p) => {
                // Send options
                if let Some(mut stdin) = p.stdin.take() {
                    if let Err(err) = stdin.write_all(self.params_json().as_bytes()) {
                        return Err(SysinspectError::ModuleError(format!("Error while communicating with the module: {err}")));
                    }
                }

                // Get the output
                if let Ok(out) = p.wait_with_output() {
                    match str::from_utf8(&out.stdout) {
                        Ok(out) => match serde_json::from_str::<ActionModResponse>(out) {
                            Ok(r) => Ok(Some(ActionResponse::new(
                                self.eid.to_owned(),
                                self.aid.to_owned(),
                                self.state.to_owned(),
                                r.clone(),
                                self.eval_constraints(&r),
                            ))),
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
        }
    }
}

impl Display for ModCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModCall - State: {}, Module: {:?}, Opts: {:?}, Args: {:?}", self.state, self.module, self.opts, self.args)?;
        Ok(())
    }
}
