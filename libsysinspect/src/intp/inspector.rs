use super::{
    actions::Action,
    checkbook::CheckbookSection,
    conf::Config,
    constraints::Constraint,
    entities::Entity,
    functions::{ClaimNamespace, ModArgFunction, StaticNamespace},
    relations::Relation,
};
use crate::{
    SysinspectError,
    cfg::mmconf::DEFAULT_MODULES_SHARELIB,
    intp::functions,
    mdescr::{
        DSL_DIR_ACTIONS, DSL_DIR_CONSTRAINTS, DSL_DIR_ENTITIES, DSL_DIR_RELATIONS, DSL_IDX_CFG, DSL_IDX_CHECKBOOK, DSL_IDX_EVENTS_CFG,
        mspecdef::ModelSpec,
    },
    reactor::handlers,
};
use colored::Colorize;
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use serde_yaml::{Value, to_value};
use std::{collections::HashSet, path::PathBuf};

static _SHARELIB: OnceCell<PathBuf> = OnceCell::new();

/// Set sharelib for further work
fn set_sharelib(sharelib: Option<PathBuf>) {
    // Already set
    if _SHARELIB.get().is_some() {
        return;
    }

    _ = _SHARELIB.set(sharelib.unwrap_or(PathBuf::from(DEFAULT_MODULES_SHARELIB)));
}

/// Get sharelib
pub fn get_cfg_sharelib() -> PathBuf {
    if let Some(sharelib) = _SHARELIB.get() {
        return sharelib.clone();
    }

    // Default
    set_sharelib(None);
    _SHARELIB.get().unwrap().clone()
}

pub struct SysInspector {
    entities: IndexMap<String, Entity>,
    relations: IndexMap<String, Relation>,
    actions: IndexMap<String, Action>,
    constraints: IndexMap<String, Constraint>,
    checkbook: Vec<CheckbookSection>,
    config: Config,
    spec: ModelSpec,
    context: IndexMap<String, serde_json::Value>,
    schemaonly: bool,
}

impl SysInspector {
    pub fn new(spec: ModelSpec, sharelib: Option<PathBuf>, context: IndexMap<String, serde_json::Value>) -> Result<Self, SysinspectError> {
        // Set sharelib
        set_sharelib(sharelib);

        let mut sr = SysInspector::schema(spec)?;
        sr.schemaonly = false;
        sr.context = context;

        // Load all handlers into factory
        handlers::registry::init_handlers();

        Ok(sr)
    }

    /// Used only for parse the model and navigate its structure, but doesn't actually run anything
    pub fn schema(spec: ModelSpec) -> Result<Self, SysinspectError> {
        let mut sr = SysInspector {
            entities: IndexMap::new(),
            relations: IndexMap::new(),
            actions: IndexMap::new(),
            constraints: IndexMap::new(),
            checkbook: Vec::default(),
            config: Config::default(),
            spec,
            context: IndexMap::new(),
            schemaonly: true,
        };

        sr.load()?;
        sr.validate()?;

        Ok(sr)
    }

    /// Load all objects.
    fn load(&mut self) -> Result<&mut Self, SysinspectError> {
        for directive in
            [DSL_DIR_ENTITIES, DSL_DIR_ACTIONS, DSL_DIR_CONSTRAINTS, DSL_DIR_RELATIONS, DSL_IDX_CHECKBOOK, DSL_IDX_CFG, DSL_IDX_EVENTS_CFG]
        {
            let v_obj = &self.spec.top(directive);
            if !directive.eq(DSL_DIR_CONSTRAINTS) && v_obj.is_none() {
                return Err(SysinspectError::ModelDSLError(format!("Directive '{directive}' is not defined")));
            }

            // skip if no directive
            if v_obj.is_none() {
                log::debug!("Optional directive '{}' is not defined in the model spec, skipping", directive);
                continue;
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
                            log::trace!("{obj:#?}");
                            self.actions.insert(obj.id(), obj);
                            amt += 1;
                        }
                        d if d == DSL_DIR_CONSTRAINTS => match Constraint::new(obj_id, obj_data) {
                            Ok(obj) => {
                                log::trace!("{obj:#?}");
                                self.constraints.insert(obj.id(), obj);
                                amt += 1;
                            }
                            Err(err) => {
                                log::warn!("Skipping validation rule: {err}");
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
            } else {
                log::warn!("Directive '{}' is expected to be a mapping, but it's not. Skipping.", directive);
            }

            log::debug!("Loaded {amt} instances of {directive}");
        }

        Ok(self)
    }

    /// Perform various model validations.
    fn validate(&self) -> Result<(), SysinspectError> {
        // Validate action chain: all constraints mentioned must be defined
        let csr_ids = self.constraints.iter().map(|(k, _)| k.to_owned()).collect::<Vec<String>>();
        let mut ref_csr: Vec<String> = Vec::new();
        for a in self.actions.clone().values() {
            ref_csr.append(&mut a.if_true());
            ref_csr.append(&mut a.if_false());
        }
        ref_csr.retain(|item| !csr_ids.contains(item));

        if !ref_csr.is_empty() {
            return Err(SysinspectError::ModelDSLError(format!(
                "Action chain requires definition of the following constraints: {}",
                ref_csr.join(", ")
            )));
        }

        Ok(())
    }

    /// Get actions by relations
    pub fn actions_by_relations(&self, rids: Vec<String>, state: Option<String>) -> Result<Vec<Action>, SysinspectError> {
        let mut out: Vec<Action> = Vec::default();
        for s in &self.checkbook {
            if rids.contains(&s.id()) {
                for r in s.relations() {
                    out.extend(self.actions_by_entities(r.required(&parse_state(state.clone()))?, state.clone())?);
                }
            }
        }

        if out.is_empty() {
            return Err(SysinspectError::ModelDSLError(format!(
                "Checkbook contains no such relations as \"{}\" that would be aligned with the state \"{}\"",
                rids.join(", "),
                parse_state(state)
            )));
        }

        Ok(out)
    }

    /// Get actions by entities
    pub fn actions_by_entities(&self, eids: Vec<String>, state: Option<String>) -> Result<Vec<Action>, SysinspectError> {
        let mut out: Vec<Action> = Vec::default();
        let state = parse_state(state);
        let mut dropped: HashSet<String> = HashSet::default();
        dropped.extend(eids.clone());

        for eid in eids {
            for action in self.actions.values() {
                if action.binds_to(&eid) && action.has_state(&state) {
                    log::debug!("Action entity: {} (entity: {}, state: {state})", action.id(), &eid);
                    // Actions are registered with a specific Entitiy Id (eid)
                    // Because as the same Action gets registered with the another eid,
                    // it also corresponds to other claims and conditions, and that then
                    // needs to be passed to the reactor.
                    if self.schemaonly {
                        out.push(action.to_owned());
                    } else {
                        out.push(action.to_owned().setup(self, &eid, state.to_owned())?);
                    }
                    dropped.remove(&eid);
                }
            }
        }

        if !dropped.is_empty() {
            return Err(SysinspectError::ModelDSLError(format!(
                "Entities \"{}\" are not bound with the state \"{}\" or don't exist",
                dropped.iter().map(|s| s.to_string()).collect::<Vec<String>>().join(", "),
                state
            )));
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

    /// Get all entities
    pub fn entities(&self) -> Vec<&Entity> {
        self.entities.values().collect()
    }

    /// Claim function
    pub fn call_function(&self, eid: Option<&str>, state: &str, func: &ModArgFunction) -> Result<Option<Value>, SysinspectError> {
        match func.fid() {
            "claim" | "static" => {
                if func.namespace().is_empty() {
                    return Err(SysinspectError::ModelDSLError(format!("A {} function is missing namespace", func.fid())));
                }
            }
            "context" => {
                if func.ns().len() != 1 {
                    return Err(SysinspectError::ModelDSLError(format!("A {} function cannot use a dot-notation namespace", func.fid())));
                }
            }
            _ => {
                return Err(SysinspectError::ModelDSLError(format!("Unknown function: {}", format!("{}(...)", func.fid()).bright_red())));
            }
        };

        if func.fid().eq("claim") {
            let eid = eid.unwrap_or_default();
            let entity = Entity::default();
            let entity = self.get_entity(eid).unwrap_or(&entity);
            if let Some(claims) = entity.claims() {
                // State-specific, or look for a fallback ("?") if any
                if let Some(claims) = claims.get(state).or_else(|| claims.get("?")) {
                    for claim in claims {
                        if let Some(v) = claim.get(func.ns().get(ClaimNamespace::LABEL as usize).unwrap()) {
                            return Ok(functions::get_by_namespace(Some(v).cloned(), func.ns()[1..].join(".").as_str()));
                        }
                    }
                } else {
                    return Err(SysinspectError::ModelDSLError(format!("No claims at {eid}.claims defined")));
                }
            }
        } else if func.fid().eq("context") {
            if let Some(v) = self.context.get(func.ns()[0].as_str()) {
                return Ok(Some(to_value(v).map_err(|e| SysinspectError::ModelDSLError(format!("Conversion error: {e}")))?));
            } else {
                return Err(SysinspectError::ModelDSLError(format!("Value '{}' in the context function was not found or defined", func.ns()[0])));
            }
        } else if func.fid().eq("static") {
            match func.ns().get(StaticNamespace::SECTION as usize).unwrap_or(&"".to_string()).as_str() {
                "entities" => {
                    if let Some(e) = self.entities.get(func.ns().get(StaticNamespace::ENTITY as usize).unwrap_or(&"".to_string())) {
                        // Get function state
                        let state = func.ns().get(StaticNamespace::STATE as usize).cloned();
                        if state.is_none() {
                            return Err(SysinspectError::ModelDSLError("Static function doesn't reach state of a claim".to_string()));
                        }
                        let state = state.unwrap();

                        // Get label
                        let label = func.ns().get(StaticNamespace::LABEL as usize).cloned();
                        if label.is_none() {
                            return Err(SysinspectError::ModelDSLError("Static function doesn't reach label of a claim".to_string()));
                        }
                        let label = label.unwrap();

                        if let Some(claims) = e.claims()
                            && let Some(claims) = claims.get(&state)
                        {
                            for claim in claims {
                                let ret = functions::get_by_namespace(claim.get(&label).cloned(), func.ns()[5..].join(".").as_str());
                                if ret.is_some() {
                                    return Ok(ret);
                                }
                            }
                            return Err(SysinspectError::ModelDSLError(format!("Static namespace \"{}\" is unreachable", func.namespace())));
                        }
                    }
                }
                _ => {
                    return Err(SysinspectError::ModelDSLError("Static functions currently can take data only from entities".to_string()));
                }
            }
        }

        Ok(None)
    }
}

/// Parse state or return a default one
pub fn parse_state(state: Option<String>) -> String {
    state.unwrap_or("$".to_string()).trim().to_string()
}
