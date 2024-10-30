use super::{
    actproc::{modfinder::ModCall, response::ActionResponse},
    constraints::Expression,
    functions,
    inspector::SysInspector,
};
use crate::{util::dataconv, SysinspectError};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ModArgs {
    #[serde(alias = "opts")]
    options: Option<Vec<String>>,

    #[serde(alias = "args")]
    arguments: Option<HashMap<String, String>>,
}

impl ModArgs {
    /// Return args
    pub fn args(&self) -> HashMap<String, String> {
        if let Some(args) = &self.arguments {
            return args.to_owned();
        }
        HashMap::default()
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
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Action {
    id: Option<String>, // NOTE: It is not optional, just added later!
    description: Option<String>,
    module: String,
    bind: Vec<String>,
    state: HashMap<String, ModArgs>,
    call: Option<ModCall>,
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

    /// Run action
    pub fn run(&self) -> Result<Option<ActionResponse>, SysinspectError> {
        if let Some(call) = &self.call {
            log::debug!("Calling action {} on state {}", self.id().yellow(), call.state().yellow());
            return call.run();
        }

        Ok(None)
    }

    fn resolve_claims(
        &self, v_expr: Vec<Expression>, inspector: &SysInspector, eid: &str, state: String,
    ) -> Result<Vec<Expression>, SysinspectError> {
        let mut out: Vec<Expression> = Vec::default();
        for mut expr in v_expr {
            if let Some(modfunc) = functions::is_function(&dataconv::to_string(expr.get_op()).unwrap_or_default()).ok().flatten()
            {
                match inspector.call_function(eid, &state, &modfunc) {
                    Ok(Some(v)) => expr.set_active_op(v)?,
                    Ok(_) => {}
                    Err(err) => log::error!("Error calling claim(): {}", err),
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
            let mut modcall = ModCall::default()
                .set_state(state)
                .set_module(mpath)
                .set_aid(self.id())
                .set_eid(eid.to_string())
                .set_constraints(cst);

            for (kw, arg) in &mod_args.args() {
                let mut arg = arg.to_owned();
                if let Ok(Some(func)) = functions::is_function(&arg) {
                    match inspector.call_function(eid, &modcall.state(), &func) {
                        Ok(None) => {
                            return Err(SysinspectError::ModelDSLError(format!(
                                "Entity {}.claims.{}.{} does not exist",
                                eid,
                                &modcall.state(),
                                func.namespace()
                            )))
                        }
                        Ok(Some(v)) => {
                            // XXX: Passing args to the modcall are for now always strings
                            arg = dataconv::to_string(Some(v)).unwrap_or_default();
                        }
                        Err(err) => return Err(err),
                    }
                }
                modcall.add_kwargs(kw.to_owned(), arg);
            }

            for opt in &mod_args.opts() {
                modcall.add_opt(opt.to_owned());
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
        write!(
            f,
            "<Action> - Id: {}, Descr: {}, Module: {}, Active: {}",
            self.id(),
            self.descr(),
            self.module,
            self.call.is_some()
        )?;

        Ok(())
    }
}
