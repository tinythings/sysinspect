use super::{
    actproc::{modfinder::ModCall, response::ActionResponse},
    constraints::Expression,
    functions,
    inspector::SysInspector,
};
use crate::{SysinspectError, logger::log_forward, util::dataconv};
use colored::Colorize;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ModArgs {
    #[serde(alias = "opts")]
    options: Option<Vec<String>>,

    #[serde(alias = "args")]
    arguments: Option<IndexMap<String, Value>>,

    #[serde(alias = "ctx")]
    context: Option<IndexMap<String, String>>, // Context variables definition for Jinja templates. Used only for model documentation.

    #[serde(alias = "conds")]
    conditions: Option<IndexMap<String, Value>>, // Conditions to be met for this state
}

impl ModArgs {
    /// Return args
    pub fn args(&self) -> IndexMap<String, Value> {
        if let Some(args) = &self.arguments {
            return args.to_owned();
        }
        IndexMap::default()
    }

    /// Get options
    pub fn opts(&self) -> Vec<String> {
        let mut out = Vec::<String>::default();
        if let Some(optset) = &self.options {
            for opt in optset {
                out.push(opt.to_owned());
            }
        }
        out
    }

    /// Get context variables
    pub fn context(&self) -> IndexMap<String, String> {
        self.context.to_owned().unwrap_or_default()
    }

    /// Get conditions
    pub fn conditions(&self) -> IndexMap<String, Value> {
        self.conditions.to_owned().unwrap_or_default()
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Action {
    id: Option<String>, // NOTE: It is not optional, just added later!
    description: Option<String>,
    module: String,
    bind: Vec<String>,
    state: IndexMap<String, ModArgs>,
    call: Option<ModCall>,

    #[serde(rename = "if-true")]
    if_true: Option<Vec<String>>,

    #[serde(rename = "if-false")]
    if_false: Option<Vec<String>>,
}

impl Action {
    pub fn new(id: &Value, states: &Value) -> Result<Self, SysinspectError> {
        let i_id: String;

        if let Some(id) = id.as_str() {
            i_id = id.to_string();
        } else {
            return Err(SysinspectError::ModelDSLError("No id found for an action".to_string()));
        }

        if let Ok(mut i) = serde_yaml::from_value::<Action>(states.to_owned()) {
            i.id = Some(i_id);
            Ok(i)
        } else {
            Err(SysinspectError::ModelDSLError(format!("Action {i_id} is misconfigured")))
        }
    }

    /// Get a list of constraints those should be true
    pub fn if_true(&self) -> Vec<String> {
        self.if_true.clone().unwrap_or_default()
    }

    /// Get a list of constraints those should be false
    pub fn if_false(&self) -> Vec<String> {
        self.if_false.clone().unwrap_or_default()
    }

    /// Get action's `id` field
    pub fn id(&self) -> String {
        self.id.to_owned().unwrap_or("".to_string())
    }

    /// Get action's `description` field
    pub fn descr(&self) -> String {
        self.description.to_owned().unwrap_or(format!("Action {}", self.id()))
    }

    /// Returns true if an action has a bind to an entity via its `eid` _(entity Id)_.
    pub fn binds_to(&self, eid: &str) -> bool {
        self.bind.contains(&eid.to_string())
    }

    /// Returns true if an action has requested state and is eligible to be processed.
    pub fn has_state(&self, sid: &str) -> bool {
        self.state.contains_key(sid)
    }

    /// Get all states defined for an action
    pub fn states(&self, default: Option<String>) -> Vec<(String, ModArgs)> {
        self.state
            .iter()
            .map(|(k, v)| {
                let key = if k == "$" { default.clone().unwrap_or_else(|| "$".to_string()) } else { k.clone() };
                (key, v.clone())
            })
            .collect()
    }

    /// Run action
    pub fn run(&self, forward_logs: bool) -> Result<Option<ActionResponse>, SysinspectError> {
        if let Some(call) = &self.call {
            log::debug!("Calling action {} on state {}", self.id().yellow(), call.state().yellow());
            let r: Option<ActionResponse> = match call.run() {
                Ok(mut r_opt) => {
                    if let Some(ref mut r) = r_opt
                        && let Some(mut data) = r.response.data()
                    {
                        if forward_logs
                            && let serde_json::Value::Object(ref mut map) = data
                            && let Some(logs_val) = map.remove("logs")
                        {
                            // forward captured logs
                            match logs_val {
                                serde_json::Value::Array(items) => {
                                    for item in items {
                                        if let Some(line) = item.as_str() {
                                            if forward_logs {
                                                log_forward(line);
                                            }
                                        } else if forward_logs {
                                            log_forward(&dataconv::as_str(Some(item)));
                                        }
                                    }
                                }
                                other => {
                                    if forward_logs {
                                        log_forward(&dataconv::as_str(Some(other)));
                                    }
                                }
                            }
                        }

                        r.response.set_data(data);
                    }

                    r_opt
                }
                Err(err) => {
                    return Err(SysinspectError::ModelDSLError(format!("Action {} failed to run: {}", self.id(), err)));
                }
            };
            return Ok(r);
        }

        Ok(None)
    }

    fn resolve_claims(
        &self, v_expr: Vec<Expression>, inspector: &SysInspector, eid: &str, state: String,
    ) -> Result<Vec<Expression>, SysinspectError> {
        let mut out: Vec<Expression> = Vec::default();
        for mut expr in v_expr {
            if let Some(modfunc) = functions::is_function(&expr.get_op().unwrap_or(Value::String("".to_string()))).ok().flatten() {
                match inspector.call_function(Some(eid), &state, &modfunc) {
                    Ok(Some(v)) => expr.set_active_op(v)?,
                    Ok(_) => {}
                    Err(err) => log::error!("Data function error: {err}"),
                }
            }
            out.push(expr);
        }

        Ok(out)
    }

    /// Setup and activate an action and is done by the Inspector.
    /// This method finds module, sets up its parameters, binds constraint etc.
    pub(crate) fn setup(&mut self, inspector: &SysInspector, eid: &str, state: String) -> Result<Action, SysinspectError> {
        let mpath = inspector.cfg().get_module(&self.module)?;

        /*
        XXX: Bogus constraints are still present in the whole pool.

            When inspector.constraints() is called, it returns a vector of *active* constraints.
            Once the claims are resolved in an expression, then that expression is pushed back
            into constraint by calling constraint.set_expr_for(..).

            However, the general pool of all constraints that is coming from the configuration
            still contains constraints that never will be evaluated anyway in this round.
            This is done by "resolve_claims()" function, which resolves only to the current
            state, withing THIS "setup()" function (see var "state").

            Either it makes sense to fully remove them within the session or evaluate all of them?
         */
        if let Some(mod_args) = self.state.get(&state) {
            // Call functions for constraints

            let mut cst = inspector.constraints(Some(self.id()), &self.bind);
            for c in &mut cst {
                // all
                c.set_expr_for(
                    state.to_owned(),
                    self.resolve_claims(c.all(state.to_owned()), inspector, eid, state.to_owned())?,
                    crate::intp::constraints::ConstraintKind::All,
                );

                // any
                c.set_expr_for(
                    state.to_owned(),
                    self.resolve_claims(c.any(state.to_owned()), inspector, eid, state.to_owned())?,
                    crate::intp::constraints::ConstraintKind::Any,
                );

                // none
                c.set_expr_for(
                    state.to_owned(),
                    self.resolve_claims(c.none(state.to_owned()), inspector, eid, state.to_owned())?,
                    crate::intp::constraints::ConstraintKind::None,
                );
            }

            // Setup modcall
            let mut modcall = ModCall::default().set_state(state).set_module(mpath).set_aid(self.id()).set_eid(eid.to_string()).set_constraints(cst);

            // Set module launching arguments
            for (kw, arg) in &mod_args.args() {
                let mut arg = arg.to_owned();
                if let Ok(Some(func)) = functions::is_function(&arg) {
                    match inspector.call_function(Some(eid), &modcall.state(), &func) {
                        Ok(None) => {
                            return Err(SysinspectError::ModelDSLError(format!(
                                "Entity {}.claims.{}.{} does not exist",
                                eid,
                                &modcall.state(),
                                func.namespace()
                            )));
                        }
                        Ok(Some(v)) => {
                            arg = v;
                        }
                        Err(err) => return Err(err),
                    }
                }
                modcall.add_kwargs(kw.to_owned(), arg);
            }

            // Set module launching options
            for opt in &mod_args.opts() {
                modcall.add_opt(opt.to_owned());
            }

            // Add module launching conditions
            for (kw, cond) in mod_args.conditions() {
                modcall.add_condition(kw, cond.clone());
            }

            self.call = Some(modcall);
        } else {
            return Err(SysinspectError::ModelDSLError(format!(
                "Action \"{}\" passes no entity claims to the module \"{}\"",
                self.id(),
                self.module
            )));
        }
        Ok(self.to_owned())
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<Action> - Id: {}, Descr: {}, Module: {}, Active: {}", self.id(), self.descr(), self.module, self.call.is_some())?;

        Ok(())
    }
}
