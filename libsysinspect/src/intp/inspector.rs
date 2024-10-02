use super::{actions::Action, constraints::Constraint, entities::Entity, relations::Relation};
use crate::{
    mdl::{mspecdef::ModelSpec, DSL_DIR_ACTIONS, DSL_DIR_CONSTRAINTS, DSL_DIR_ENTITIES, DSL_DIR_RELATIONS},
    SysinspectError,
};
use std::collections::HashMap;

pub struct SysInspector {
    entities: HashMap<String, Entity>,
    relations: HashMap<String, Relation>,
    actions: HashMap<String, Action>,
    constraints: HashMap<String, Constraint>,
}

impl SysInspector {
    pub fn new(spec: &ModelSpec) -> Result<Self, SysinspectError> {
        let mut sr = SysInspector {
            entities: HashMap::new(),
            relations: HashMap::new(),
            actions: HashMap::new(),
            constraints: HashMap::new(),
        };
        sr.load(spec)?;

        Ok(sr)
    }

    /// Load all objects.
    fn load(&mut self, spec: &ModelSpec) -> Result<&mut Self, SysinspectError> {
        for directive in [DSL_DIR_ENTITIES, DSL_DIR_ACTIONS, DSL_DIR_CONSTRAINTS, DSL_DIR_RELATIONS] {
            let obj = spec.top(directive);
            if !directive.eq(DSL_DIR_CONSTRAINTS) && obj.is_none() {
                return Err(SysinspectError::ModelDSLError(format!("Directive '{directive}' is not defined")));
            }

            let mut amt = 0;
            if let Some(obj) = obj.unwrap().as_mapping() {
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
                        _ => {}
                    }
                }
            }

            log::debug!("Loaded {amt} instances of {directive}");
        }

        Ok(self)
    }
}
