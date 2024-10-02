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
        sr.load_entities(spec)?.load_relations(spec)?.load_actions(spec)?.load_constraints(spec)?;

        Ok(sr)
    }

    /// Load all entities
    fn load_entities(&mut self, spec: &ModelSpec) -> Result<&mut Self, SysinspectError> {
        let e = spec.top(DSL_DIR_ENTITIES);
        if e.is_none() {
            return Err(SysinspectError::ModelDSLError("No defined entities has been found".to_string()));
        }

        let mut amt = 0;
        for v_ent in e.unwrap().as_sequence().unwrap_or(&vec![]) {
            let e = Entity::new(v_ent)?;
            self.entities.insert(e.id(), e);
            amt += 1;
        }

        log::debug!("Loaded {amt} entities");

        Ok(self)
    }

    /// Load all relations
    fn load_relations(&mut self, spec: &ModelSpec) -> Result<&mut Self, SysinspectError> {
        let r = spec.top(DSL_DIR_RELATIONS);
        if r.is_none() {
            return Err(SysinspectError::ModelDSLError("No relations between entities defined".to_string()));
        }

        let mut amt = 0;
        if let Some(r) = r.unwrap().as_mapping() {
            for (v_id, v_states) in r {
                let rel = Relation::new(v_id, v_states)?;
                self.relations.insert(rel.id(), rel);
                amt += 1;
            }
        } else {
            return Err(SysinspectError::ModelDSLError("Syntax error in relations: key/value structure is expected".to_string()));
        }

        log::debug!("Loaded {amt} relations");
        Ok(self)
    }

    /// Load all actions
    fn load_actions(&mut self, spec: &ModelSpec) -> Result<&mut Self, SysinspectError> {
        let a = spec.top(DSL_DIR_ACTIONS);
        if a.is_none() {
            return Err(SysinspectError::ModelDSLError("No actions defined, therefore no processing is possible".to_string()));
        }

        let mut amt = 0;
        if let Some(a) = a.unwrap().as_mapping() {
            for (v_id, v_states) in a {
                let act = Action::new(v_id, v_states)?;
                self.actions.insert(act.id(), act);
                amt += 1;
            }
        } else {
            return Err(SysinspectError::ModelDSLError(
                "Syntax error in actions. Please check it, referring to the documentation.".to_string(),
            ));
        }

        log::debug!("Loaded {amt} actions");
        Ok(self)
    }

    fn load_constraints(&mut self, spec: &ModelSpec) -> Result<&mut Self, SysinspectError> {
        let c = spec.top(DSL_DIR_CONSTRAINTS);
        if c.is_none() {
            // Constraints are optional
            return Ok(self);
        }

        let mut amt = 0;
        if let Some(c) = c.unwrap().as_mapping() {
            for (v_id, v_cst) in c {
                let cst = Constraint::new(v_id, v_cst)?;
                self.constraints.insert(cst.id(), cst);

                amt += 1;
            }
        } else {
            return Err(SysinspectError::ModelDSLError(
                "Syntax error in constraints: key/value structure is expected".to_string(),
            ));
        }

        log::debug!("Loaded {amt} constraints");
        Ok(self)
    }
}
