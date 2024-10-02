use super::{actions::Action, constraints::Constraint, entities::Entity, relations::Relation};
use crate::{mdl::mspecdef::ModelSpec, SysinspectError};
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
        sr.load_entities(spec)?.load_relations(spec)?.load_actions(spec)?.load_constraints(spec)?;

        Ok(sr)
    }

    /// Load all entities
    fn load_entities(&mut self, spec: &ModelSpec) -> Result<&mut Self, SysinspectError> {
        let e = spec.top("entities");
        if e.is_none() {
            return Err(SysinspectError::ModelDSLError("No defined entities has been found".to_string()));
        }

        let mut amt = 0;
        for v_ent in e.unwrap().as_sequence().unwrap_or(&vec![]) {
            match Entity::new(v_ent) {
                Ok(e) => {
                    self.entities.insert(e.id(), e);
                }
                Err(err) => return Err(err),
            }
            amt += 1;
        }

        log::debug!("Loaded {amt} entities");

        Ok(self)
    }

    /// Load all relations
    fn load_relations(&mut self, spec: &ModelSpec) -> Result<&mut Self, SysinspectError> {
        log::debug!("Loading relations");
        Ok(self)
    }

    /// Load all actions
    fn load_actions(&mut self, spec: &ModelSpec) -> Result<&mut Self, SysinspectError> {
        log::debug!("Loading actions");
        Ok(self)
    }

    fn load_constraints(&mut self, spec: &ModelSpec) -> Result<&mut Self, SysinspectError> {
        log::debug!("Loading constraints");
        Ok(self)
    }
}
