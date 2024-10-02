use super::{actions::Action, constraints::Constraint, entities::Entity, relations::Relation};
use crate::mdl::mspecdef::ModelSpec;
use std::collections::HashMap;

pub struct SysInspector {
    entities: HashMap<String, Entity>,
    relations: HashMap<String, Relation>,
    actions: HashMap<String, Action>,
    constraints: HashMap<String, Constraint>,
}

impl SysInspector {
    pub fn new(spec: &ModelSpec) -> Self {
        let mut sr = SysInspector {
            entities: HashMap::new(),
            relations: HashMap::new(),
            actions: HashMap::new(),
            constraints: HashMap::new(),
        };
        sr.load_entities(spec).load_relations(spec).load_actions(spec).load_constraints(spec);

        sr
    }

    /// Load all entities
    fn load_entities(&mut self, spec: &ModelSpec) -> &mut Self {
        log::debug!("Loading entities");
        let e = spec.top("entities");
        if e.is_none() {
            return self;
        }
        for v_ent in e.unwrap().as_sequence().unwrap_or(&vec![]) {
            let e = Entity::new(v_ent);
            println!("{:?}", e);
            println!("\n{:?}", v_ent);
            println!("---------------------------");
        }

        self
    }

    /// Load all relations
    fn load_relations(&mut self, spec: &ModelSpec) -> &mut Self {
        log::debug!("Loading relations");
        self
    }

    /// Load all actions
    fn load_actions(&mut self, spec: &ModelSpec) -> &mut Self {
        log::debug!("Loading actions");
        self
    }

    fn load_constraints(&mut self, spec: &ModelSpec) -> &mut Self {
        log::debug!("Loading constraints");
        self
    }
}
