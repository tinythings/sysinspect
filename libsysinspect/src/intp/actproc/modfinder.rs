use super::response::{ActionModResponse, ActionResponse};
use crate::{
    intp::{
        actproc::get_by_ns,
        constraints::{Constraint, Expression},
    },
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

    /// All expressions must be true
    fn eval_cst_all(&self, cstr: &Constraint, resp: &ActionModResponse) -> Option<bool> {
        let exp = cstr.all(self.state());
        if exp.is_empty() {
            return None;
        }

        for exp in exp {
            if !exp.eval(Expression::get_by_namespace(resp.data(), &exp.get_fact_namespace())) {
                return Some(false);
            }
        }

        Some(true)
    }

    /// At least one of the expressions must be true
    fn eval_cst_any(&self, cstr: &Constraint, resp: &ActionModResponse) -> Option<bool> {
        let exp = cstr.any(self.state());
        if exp.is_empty() {
            return None;
        }

        for exp in exp {
            if exp.eval(Expression::get_by_namespace(resp.data(), &exp.get_fact_namespace())) {
                return Some(true);
            }
        }

        Some(false)
    }

    /// None of expressions should be true. It is basically !all.
    fn eval_cst_none(&self, cstr: &Constraint, resp: &ActionModResponse) -> Option<bool> {
        let exp = cstr.none(self.state());
        if exp.is_empty() {
            return None;
        }

        for e in exp {
            if e.eval(Expression::get_by_namespace(resp.data(), &e.get_fact_namespace())) {
                return Some(false);
            }
        }

        Some(true)
    }

    /// Evaluate constraints
    fn eval_constraints(&self, ar: &ActionModResponse) {
        for c in &self.constraints {
            println!("Evaluating: {}", c.descr());
            if let Some(r) = self.eval_cst_all(c, ar) {
                println!("  All: {}", if r { "OK" } else { "FAILED" });
            }

            if let Some(r) = self.eval_cst_any(c, ar) {
                println!("  Any: {}", if r { "OK" } else { "FAILED" });
            }

            if let Some(r) = self.eval_cst_none(c, ar) {
                println!("  None: {}", if r { "OK" } else { "FAILED" });
            }
        }
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
                            Ok(r) => {
                                self.eval_constraints(&r);
                                Ok(Some(ActionResponse::new(self.eid.to_owned(), self.aid.to_owned(), self.state.to_owned(), r)))
                            }
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
