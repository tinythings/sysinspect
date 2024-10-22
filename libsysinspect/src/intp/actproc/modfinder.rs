use super::response::{ActionModResponse, ActionResponse, ConstraintResponse};
use crate::{
    intp::{
        actproc::response::ConstraintFailure,
        constraints::{Constraint, ConstraintKind, Expression},
    },
    util::dataconv,
    SysinspectError,
};
use core::str;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    fmt::Display,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
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
    args: HashMap<String, String>,
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
        let mut out: HashMap<String, serde_json::Value> = HashMap::default();
        if !self.args.is_empty() {
            out.insert("arguments".to_string(), json!(self.args));
        }

        if !self.opts.is_empty() {
            out.insert("options".to_string(), json!(self.opts));
        }

        let x = json!(out).to_string();
        log::trace!("Params: {}", x);
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
    fn eval_cst_all(&self, cstr: &Constraint, resp: &ActionModResponse) -> (Option<bool>, Option<String>) {
        let exp = cstr.all(self.state());
        if exp.is_empty() {
            return (None, None);
        }

        for exp in exp {
            let fact = Expression::get_by_namespace(resp.data(), &exp.get_fact_namespace());
            if !exp.eval(fact.to_owned()) {
                return (
                    Some(false),
                    Some(format!("{} fails with {}", &exp.get_fact_namespace(), dataconv::to_string(fact).unwrap_or_default())),
                );
            }
        }

        (Some(true), None)
    }

    /// At least one of the expressions must be true
    fn eval_cst_any(&self, cstr: &Constraint, resp: &ActionModResponse) -> (Option<bool>, Option<String>) {
        let exp = cstr.any(self.state());
        if exp.is_empty() {
            return (None, None);
        }

        for exp in exp {
            if exp.eval(Expression::get_by_namespace(resp.data(), &exp.get_fact_namespace())) {
                return (Some(true), None);
            }
        }

        (Some(false), Some("No constraints matches found".to_string()))
    }

    /// None of expressions should be true. It is basically !all.
    fn eval_cst_none(&self, cstr: &Constraint, resp: &ActionModResponse) -> (Option<bool>, Option<String>) {
        let exp = cstr.none(self.state());
        if exp.is_empty() {
            return (None, None);
        }

        for e in exp {
            let fact = Expression::get_by_namespace(resp.data(), &e.get_fact_namespace());
            if e.eval(fact.to_owned()) {
                return (
                    Some(false),
                    Some(format!("{} fails with {}", &e.get_fact_namespace(), dataconv::to_string(fact).unwrap_or_default())),
                );
            }
        }

        (Some(true), None)
    }

    /// Evaluate constraints
    fn eval_constraints(&self, ar: &ActionModResponse) -> ConstraintResponse {
        fn eval<F>(
            mc: &ModCall, cret: &mut ConstraintResponse, c: &Constraint, kind: ConstraintKind, eval_fn: F, ar: &ActionModResponse,
        ) where
            F: Fn(&ModCall, &Constraint, &ActionModResponse) -> (Option<bool>, Option<String>),
        {
            let (res, msg) = eval_fn(mc, c, ar);
            if let Some(res) = res {
                if !res {
                    cret.add_failure(ConstraintFailure::new(c.descr(), msg.unwrap_or_default(), kind));
                }
            }
        }

        let mut cret =
            ConstraintResponse::new(format!("{} with {}", self.aid, self.get_mod_ns().unwrap_or("(unknown)".to_string())));
        for c in &self.constraints {
            eval(&self, &mut cret, c, ConstraintKind::All, Self::eval_cst_all, ar);
            eval(&self, &mut cret, c, ConstraintKind::Any, Self::eval_cst_any, ar);
            eval(&self, &mut cret, c, ConstraintKind::None, Self::eval_cst_none, ar);
        }

        cret
    }

    pub fn run(&self) -> Result<Option<ActionResponse>, SysinspectError> {
        //   Event reactor:
        //   - Configurable
        //   - Chain plugins/functions
        //   - Event reactions
        //   - Should probably store all the result in a common structure
        match Command::new(&self.module).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
            Ok(mut p) => {
                // Send options
                if let Some(mut stdin) = p.stdin.take() {
                    if let Err(err) = stdin.write_all(self.params_json().as_bytes()) {
                        return Err(SysinspectError::ModuleError(format!("Error while communicating with the module: {}", err)));
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
                            Err(e) => Err(SysinspectError::ModuleError(format!("JSON error: {e}"))),
                        },
                        Err(err) => Err(SysinspectError::ModuleError(format!("Error obtaining the output: {err}"))),
                    }
                } else {
                    Err(SysinspectError::ModuleError("Module returned no output".to_string()))
                }
            }
            Err(err) => Err(SysinspectError::ModuleError(format!("Error calling module: {}", err))),
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
            args: HashMap::default(),
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
