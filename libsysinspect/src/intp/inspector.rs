use super::{
    actions::Action, checkbook::CheckbookSection, conf::Config, constraints::Constraint, entities::Entity,
    functions::ModArgFunction, relations::Relation,
};
use crate::{
    mdescr::{
        mspecdef::ModelSpec, DSL_DIR_ACTIONS, DSL_DIR_CONSTRAINTS, DSL_DIR_ENTITIES, DSL_DIR_RELATIONS, DSL_IDX_CFG,
        DSL_IDX_CHECKBOOK, DSL_IDX_EVENTS_CFG,
    },
    reactor::handlers,
    SysinspectError,
};
use colored::Colorize;
use serde_yaml::Value;
use std::collections::HashMap;

pub struct SysInspector {
    entities: HashMap<String, Entity>,
    relations: HashMap<String, Relation>,
    actions: HashMap<String, Action>,
    constraints: HashMap<String, Constraint>,
    checkbook: Vec<CheckbookSection>,
    config: Config,
    spec: ModelSpec,
}

impl SysInspector {
    pub fn new(spec: ModelSpec) -> Result<Self, SysinspectError> {
        let mut sr = SysInspector {
            entities: HashMap::new(),
            relations: HashMap::new(),
            actions: HashMap::new(),
            constraints: HashMap::new(),
            checkbook: Vec::default(),
            config: Config::default(),
            spec,
        };

        sr.load()?;

        // Load all handlers into factory
        handlers::registry::init_handlers();

        Ok(sr)
    }

    /// Load all objects.
    fn load(&mut self) -> Result<&mut Self, SysinspectError> {
        for directive in [
            DSL_DIR_ENTITIES,
            DSL_DIR_ACTIONS,
            DSL_DIR_CONSTRAINTS,
            DSL_DIR_RELATIONS,
            DSL_IDX_CHECKBOOK,
            DSL_IDX_CFG,
            DSL_IDX_EVENTS_CFG,
        ] {
            let v_obj = &self.spec.top(directive);
            if !directive.eq(DSL_DIR_CONSTRAINTS) && v_obj.is_none() {
                return Err(SysinspectError::ModelDSLError(format!("Directive '{directive}' is not defined")));
            }

            let mut amt = 0;
            if let Some(obj) = v_obj.unwrap().as_mapping() {
                for (obj_id, obj_data) in obj {
                    match directive {
                        d if d == DSL_DIR_ENTITIES => {
                            let obj = Entity::new(obj_id, obj_data)?;
                            self.entities.insert(obj.id(), obj);
                            amt += 1;
                        }
                        d if d == DSL_DIR_ACTIONS => {
                            let obj = Action::new(obj_id, obj_data)?;
                            self.actions.insert(obj.id(), obj);
                            amt += 1;
                        }
                        d if d == DSL_DIR_CONSTRAINTS => match Constraint::new(obj_id, obj_data) {
                            Ok(obj) => {
                                log::trace!("{:#?}", obj);
                                self.constraints.insert(obj.id(), obj);
                                amt += 1;
                            }
                            Err(err) => {
                                log::warn!("Skipping validation rule: {}", err);
                            }
                        },
                        d if d == DSL_DIR_RELATIONS => {
                            let obj = Relation::new(obj_id, obj_data)?;
                            self.relations.insert(obj.id(), obj);
                            amt += 1;
                        }
                        d if d == DSL_IDX_CHECKBOOK => {
                            if let Some(cs) = CheckbookSection::new(obj_id, obj_data, &self.relations) {
                                self.checkbook.push(cs);
                                amt += 1;
                            }
                        }
                        _ => {}
                    }
                }

                // Load config
                if directive == DSL_IDX_CFG {
                    self.config = Config::new(v_obj.unwrap())?;
                }

                if directive == DSL_IDX_EVENTS_CFG {
                    self.config.set_events(v_obj.unwrap())?;
                }
            }

            log::debug!("Loaded {amt} instances of {directive}");
        }

        Ok(self)
    }

    /// Get actions by relations
    pub fn actions_by_relations(&self, rids: Vec<String>, state: Option<String>) -> Result<Vec<Action>, SysinspectError> {
        let mut out: Vec<Action> = Vec::default();
        for s in &self.checkbook {
            if rids.contains(&s.id()) {
                for rel in s.relations() {
                    out.extend(self.actions_by_entities(rel.get_entities(state.to_owned()), state.to_owned())?);
                }
            }
        }

        Ok(out)
    }

    /// Get actions by entities
    pub fn actions_by_entities(&self, eids: Vec<String>, state: Option<String>) -> Result<Vec<Action>, SysinspectError> {
        let mut out: Vec<Action> = Vec::default();
        let state = parse_state(state);

        for eid in eids {
            for action in self.actions.values() {
                if action.binds_to(&eid) && action.has_state(&state) {
                    log::debug!("Action entity: {} (entity: {}, state: {state})", action.id(), &eid);
                    // Actions are registered with a specific Entitiy Id (eid)
                    // Because as the same Action gets registered with the another eid,
                    // it also corresponds to other claims and conditions, and that then
                    // needs to be passed to the reactor.
                    out.push(action.to_owned().setup(self, &eid, state.to_owned())?);
                }
            }
        }

        Ok(out)
    }

    /// Return config reference
    pub fn cfg(&self) -> &Config {
        &self.config
    }

    /// Return constraints for an action by Id, or all if `aid` equals `None`.
    /// - `aid` is Action Id
    /// - `a_eids` is a list of Action's Entities Ids, those are listed in the action as bind
    pub fn constraints(&self, aid: Option<String>, a_eids: &Vec<String>) -> Vec<Constraint> {
        let mut out: Vec<Constraint> = Vec::default();
        if let Some(aid) = aid {
            for ctr in self.constraints.values() {
                if ctr.id().eq(&aid) && ctr.binds_to_any(a_eids) {
                    out.push(ctr.to_owned());
                }
            }
        } else {
            out.extend(self.constraints.values().map(|c| c.to_owned()).collect::<Vec<Constraint>>());
        }

        out
    }

    /// Get an entity definition
    pub fn get_entity(&self, eid: &str) -> Option<&Entity> {
        self.entities.get(eid)
    }

    /// Claim function

    pub fn call_function(&self, eid: &str, state: &str, func: &ModArgFunction) -> Result<Option<Value>, SysinspectError> {
        // TODO: Add support for static functions
        //
        // XXX: Functions are returning only strings.
        //      This need to change: call_function should return Value instead

        match func.fid() {
            "claim" => {}
            _ => {
                return Err(SysinspectError::ModelDSLError(format!(
                    "Unknown claim function: {}",
                    format!("{}(...)", func.fid()).bright_red()
                )))
            }
        };

        let entity = Entity::default();
        let entity = self.get_entity(eid).unwrap_or(&entity);

        if let Some(claims) = entity.claims() {
            if let Some(claims) = claims.get(state) {
                for claim in claims {
                    if let Some(v) = claim.get(func.ns_parts().unwrap()[0]) {
                        if let serde_yaml::Value::Mapping(v) = v {
                            if let Some(v) = v.get(func.ns_parts().unwrap()[1]) {
                                return Ok(Some(v).cloned());
                            }
                        } else {
                            return Err(SysinspectError::ModelDSLError(format!(
                                "Claim {}.claims.{}.{} must be a key/value mapping",
                                eid,
                                state,
                                func.namespace()
                            )));
                        }
                    }
                }
            } else {
                return Err(SysinspectError::ModelDSLError(format!("No claims at {}.claims defined", eid)));
            }
        }

        Ok(None)
    }

    /// Static function
    pub fn function_static(&self) {}
}

/// Parse state or return a default one
pub fn parse_state(state: Option<String>) -> String {
    state.unwrap_or("$".to_string()).trim().to_string()
}
