use super::{
    actions::Action, checkbook::CheckbookSection, conf::Config, constraints::Constraint, entities::Entity, relations::Relation,
};
use crate::{
    mdescr::{
        mspecdef::ModelSpec, DSL_DIR_ACTIONS, DSL_DIR_CONSTRAINTS, DSL_DIR_ENTITIES, DSL_DIR_RELATIONS, DSL_IDX_CFG,
        DSL_IDX_CHECKBOOK,
    },
    SysinspectError,
};
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

        Ok(sr)
    }

    /// Load all objects.
    fn load(&mut self) -> Result<&mut Self, SysinspectError> {
        for directive in
            [DSL_DIR_ENTITIES, DSL_DIR_ACTIONS, DSL_DIR_CONSTRAINTS, DSL_DIR_RELATIONS, DSL_IDX_CHECKBOOK, DSL_IDX_CFG]
        {
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
                        d if d == DSL_DIR_CONSTRAINTS => {
                            let obj = Constraint::new(obj_id, obj_data)?;
                            self.constraints.insert(obj.id(), obj);
                            amt += 1;
                        }
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
            }

            log::debug!("Loaded {amt} instances of {directive}");
        }

        Ok(self)
    }

    /// Get actions by relations
    pub fn actions_by_relations(&self, rids: Vec<String>) -> Result<Vec<Action>, SysinspectError> {
        Ok(vec![])
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
                    // it also corresponds to other facts and conditions, and that then
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
}

/// Parse state or return a default one
pub fn parse_state(state: Option<String>) -> String {
    state.unwrap_or("$".to_string()).trim().to_string()
}
