use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use regex::Regex;
use std::sync::LazyLock;

static CTX_FN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"context\((\w+)\)").unwrap());

use crate::{
    cfg::mmconf::MinionConfig,
    intp::{actions::Action, entities::Entity, relations::Relation},
    mdescr::{DSL_DIR_ACTIONS, DSL_DIR_ENTITIES, DSL_DIR_RELATIONS, DSL_IDX_CHECKBOOK},
};

use super::{browse_types::*, mspec, mspecdef::ModelSpec};

/// Read-only browser for one model.
///
/// Built on `ModelSpec` loaded by `mspec::load()`.  The browser
/// extracts declared model structure directly; it does **not** use
/// runtime-oriented interpreter APIs such as `actions_by_entities()`.
#[derive(Debug)]
pub struct ModelBrowser {
    spec: ModelSpec,
    model_path: PathBuf,
}

impl ModelBrowser {
    /// Load a model from the given directory path.
    ///
    /// Internally calls `mspec::load()` to walk the directory,
    /// render Tera templates, and merge the model spec tree.
    pub fn load(cfg: Arc<MinionConfig>, model_path: &Path) -> Result<Self, ModelBrowseError> {
        let model_path = model_path.canonicalize().map_err(|e| {
            ModelBrowseError::LoadError(libcommon::SysinspectError::ModelDSLError(format!("Cannot resolve model path {}: {e}", model_path.display())))
        })?;

        let spec = mspec::load(
            cfg,
            model_path
                .to_str()
                .ok_or_else(|| ModelBrowseError::LoadError(libcommon::SysinspectError::ModelDSLError("Model path is not valid UTF-8".to_string())))?,
            None,
            None,
        )?;

        Ok(Self { spec, model_path })
    }

    /// Return metadata for the loaded model.
    pub fn metadata(&self) -> BrowsedModelMetadata {
        let id = self.model_path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();

        BrowsedModelMetadata {
            id,
            path: self.model_path.clone(),
            name: self.spec.name().to_string(),
            version: self.spec.version().to_string(),
            description: self.spec.description().to_string(),
            maintainer: self.spec.maintainer().to_string(),
        }
    }

    /// Return all declared entities from the model, with diagnostics for
    /// individual entities that could not be parsed.
    pub fn entities(&self) -> (Vec<BrowsedEntity>, Vec<ModelBrowseDiagnostic>) {
        let mut diagnostics = Vec::new();

        let Some(section) = self.spec.top(DSL_DIR_ENTITIES) else {
            return (Vec::new(), diagnostics);
        };

        if let Some(seq) = section.as_sequence() {
            diagnostics.push(ModelBrowseDiagnostic {
                level: ModelBrowseDiagnosticLevel::Warning,
                message: "entities section is a list format; each entry treated as a bare entity id".to_string(),
                path: Some("entities".to_string()),
            });
            let entities: Vec<BrowsedEntity> = seq
                .iter()
                .map(|v| {
                    let eid = v.as_str().unwrap_or("?");
                    if !v.is_string() {
                        diagnostics.push(ModelBrowseDiagnostic {
                            level: ModelBrowseDiagnosticLevel::Warning,
                            message: format!("Non-string entity entry in list format: {v:?}"),
                            path: Some("entities".to_string()),
                        });
                    }
                    BrowsedEntity {
                        id: eid.to_string(),
                        descr: String::new(),
                        inherits: vec![],
                        depends: vec![],
                        claim_state_keys: vec![],
                        claim_labels: vec![],
                    }
                })
                .collect();
            return (entities, diagnostics);
        }

        let Some(mapping) = section.as_mapping() else {
            return (Vec::new(), diagnostics);
        };

        let entities: Vec<BrowsedEntity> = mapping
            .iter()
            .filter_map(|(id, data)| {
                let eid = id.as_str().unwrap_or("?");

                // Detect non-mapping payloads that silently become default entities.
                if !data.is_mapping() && !data.is_null() {
                    diagnostics.push(ModelBrowseDiagnostic {
                        level: ModelBrowseDiagnosticLevel::Warning,
                        message: format!(
                            "Entity \"{eid}\" body is not a mapping (found {}), entity will appear with default values",
                            value_kind_name(data)
                        ),
                        path: Some(format!("entities.{eid}")),
                    });
                }

                match Entity::new(id, data) {
                    Ok(entity) => {
                        let claim_state_keys: Vec<String> = entity.claims().map(|cm| cm.keys().cloned().collect()).unwrap_or_default();

                        let claim_labels: Vec<String> = {
                            let mut set = std::collections::BTreeSet::new();
                            if let Some(claims) = entity.claims() {
                                for claim_vec in claims.values() {
                                    for claim in claim_vec {
                                        for k in claim.keys() {
                                            set.insert(k.clone());
                                        }
                                    }
                                }
                            }
                            set.into_iter().collect()
                        };

                        Some(BrowsedEntity {
                            id: entity.id(),
                            descr: entity.descr(),
                            inherits: entity.inherits(),
                            depends: entity.depends(),
                            claim_state_keys,
                            claim_labels,
                        })
                    }
                    Err(_) => {
                        let eid = id.as_str().unwrap_or("?");
                        diagnostics.push(ModelBrowseDiagnostic {
                            level: ModelBrowseDiagnosticLevel::Warning,
                            message: format!("Entity \"{eid}\" could not be parsed and was skipped"),
                            path: Some(format!("entities.{eid}")),
                        });
                        None
                    }
                }
            })
            .collect();

        (entities, diagnostics)
    }

    /// Return all declared relations from the model, with diagnostics for
    /// individual relations that could not be parsed.
    pub fn relations(&self) -> (Vec<BrowsedRelation>, Vec<ModelBrowseDiagnostic>) {
        let mut diagnostics = Vec::new();

        let Some(section) = self.spec.top(DSL_DIR_RELATIONS) else {
            return (Vec::new(), diagnostics);
        };

        let Some(mapping) = section.as_mapping() else {
            return (Vec::new(), diagnostics);
        };

        let relations: Vec<BrowsedRelation> = mapping
            .iter()
            .filter_map(|(id, data)| match Relation::new(id, data) {
                Ok(rel) => {
                    let states = rel
                        .states()
                        .iter()
                        .map(|(state_key, inner)| BrowsedRelationState {
                            state: state_key.clone(),
                            required_entities: inner.get("requires").cloned().unwrap_or_default(),
                        })
                        .collect();

                    Some(BrowsedRelation { id: rel.id(), states })
                }
                Err(_) => {
                    let rid = id.as_str().unwrap_or("?");
                    diagnostics.push(ModelBrowseDiagnostic {
                        level: ModelBrowseDiagnosticLevel::Warning,
                        message: format!("Relation \"{rid}\" could not be parsed and was skipped"),
                        path: Some(format!("relations.{rid}")),
                    });
                    None
                }
            })
            .collect();

        (relations, diagnostics)
    }

    /// Return all entrypoints: checkbook labels and bare entities.
    pub fn entrypoints(&self) -> (Vec<BrowsedEntrypoint>, Vec<ModelBrowseDiagnostic>) {
        let mut entrypoints: Vec<BrowsedEntrypoint> = Vec::new();
        let mut diagnostics: Vec<ModelBrowseDiagnostic> = Vec::new();

        // --- Checkbook labels ---
        if let Some(section) = self.spec.top(DSL_IDX_CHECKBOOK)
            && let Some(mapping) = section.as_mapping()
        {
            let (relations, _rel_diags) = self.relations();
            diagnostics.extend(_rel_diags);
            for (label, rel_ids_val) in mapping {
                let label = label.as_str().unwrap_or("").to_string();
                if label.is_empty() {
                    continue;
                }

                let relation_ids: Vec<String> =
                    rel_ids_val.as_sequence().map(|seq| seq.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();

                if relation_ids.is_empty() {
                    continue;
                }

                // Emit diagnostic for any referenced relation that doesn't exist.
                for rid in &relation_ids {
                    if !relations.iter().any(|r| r.id == *rid) {
                        diagnostics.push(ModelBrowseDiagnostic {
                            level: ModelBrowseDiagnosticLevel::Warning,
                            message: format!("Checkbook label \"{label}\" references missing relation \"{rid}\""),
                            path: Some(format!("checkbook.{label}")),
                        });
                    }
                }

                // Collect first-level entity IDs from reachable relations.
                let mut entity_ids: Vec<String> = Vec::new();
                for rel in &relations {
                    if relation_ids.contains(&rel.id) {
                        for st in &rel.states {
                            for eid in &st.required_entities {
                                if !entity_ids.contains(eid) {
                                    entity_ids.push(eid.clone());
                                }
                            }
                        }
                    }
                }

                entrypoints.push(BrowsedEntrypoint::CheckbookLabel { label, relation_ids, entity_ids });
            }
        }

        // --- Bare entities as entrypoints ---
        let (entities, entity_diags) = self.entities();
        diagnostics.extend(entity_diags);
        for entity in entities {
            entrypoints.push(BrowsedEntrypoint::Entity { id: entity.id, descr: entity.descr });
        }

        (entrypoints, diagnostics)
    }

    /// Return all declared actions with every declared state, with
    /// diagnostics for individual actions that could not be parsed.
    ///
    /// Named-state-only actions are preserved; no execution filtering is applied.
    pub fn actions(&self) -> (Vec<BrowsedAction>, Vec<ModelBrowseDiagnostic>) {
        let mut diagnostics = Vec::new();

        let Some(section) = self.spec.top(DSL_DIR_ACTIONS) else {
            return (Vec::new(), diagnostics);
        };

        let Some(mapping) = section.as_mapping() else {
            return (Vec::new(), diagnostics);
        };

        // Collect known entity IDs for bind-diagnostic checks.
        let (known_entities, _) = self.entities();
        let known_eids: std::collections::BTreeSet<&str> = known_entities.iter().map(|e| e.id.as_str()).collect();

        let actions: Vec<BrowsedAction> = mapping
            .iter()
            .filter_map(|(id, data)| {
                match Action::new(id, data) {
                    Ok(action) => {
                        // Emit diagnostics for binds to unknown entities.
                        for eid in action.bind_list() {
                            if !known_eids.contains(eid.as_str()) {
                                diagnostics.push(ModelBrowseDiagnostic {
                                    level: ModelBrowseDiagnosticLevel::Warning,
                                    message: format!("Action \"{}\" binds to unknown entity \"{eid}\"", action.id()),
                                    path: Some(format!("actions.{}", action.id())),
                                });
                            }
                        }

                        let states = action
                            .states(None)
                            .into_iter()
                            .map(|(state_name, mod_args)| {
                                let opts = mod_args.opts();
                                let args: Vec<(String, String)> = mod_args.args().into_iter().map(|(k, v)| (k, value_to_display(&v))).collect();
                                let mut context_vars: Vec<(String, String, bool)> = action.action_context();
                                for (k, v, r) in mod_args.context() {
                                    if !context_vars.iter().any(|(ik, _, _)| ik == &k) {
                                        context_vars.push((k, v, r));
                                    }
                                }
                                // Scan arg values for implicit context references: context(xxx)
                                for (_, val) in args.iter() {
                                    let s = val.as_str();
                                    for cap in CTX_FN_RE.captures_iter(s) {
                                        if let Some(name) = cap.get(1)
                                            && !context_vars.iter().any(|(k, _, _)| k == name.as_str())
                                        {
                                            context_vars.push((name.as_str().to_string(), String::new(), true));
                                        }
                                    }
                                }
                                let conditions = mod_args.conditions().into_iter().map(|(k, v)| (k, value_to_display(&v))).collect();

                                BrowsedActionState { state: state_name, opts, args, context_vars, conditions }
                            })
                            .collect();

                        Some(BrowsedAction {
                            action_id: action.id(),
                            description: action.descr(),
                            module: action.module().to_string(),
                            binds_to: action.bind_list().to_vec(),
                            states,
                        })
                    }
                    Err(_) => {
                        let aid = id.as_str().unwrap_or("?");
                        diagnostics.push(ModelBrowseDiagnostic {
                            level: ModelBrowseDiagnosticLevel::Warning,
                            message: format!("Action \"{aid}\" could not be parsed and was skipped"),
                            path: Some(format!("actions.{aid}")),
                        });
                        None
                    }
                }
            })
            .collect();

        (actions, diagnostics)
    }

    /// Return the deduplicated set of all action state keys declared in
    /// the model.  Includes `$` by default. Consumers that want to hide
    /// the wildcard can filter it out.
    pub fn states(&self) -> Vec<String> {
        let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        let (actions, _) = self.actions();
        for action in &actions {
            for st in &action.states {
                set.insert(st.state.clone());
            }
        }

        set.into_iter().collect()
    }

    /// Build a complete `BrowsedModel` summary in one call.
    ///
    /// Composes metadata, entities, relations, entrypoints, actions,
    /// states, and all accumulated diagnostics into a single struct
    /// suitable for direct consumption by TUI, CLI, or other frontends.
    /// Diagnostics are deduplicated so nested helper calls do not
    /// repeat the same warning.
    pub fn summarize(&self) -> Result<BrowsedModel, ModelBrowseError> {
        let mut diagnostics: Vec<ModelBrowseDiagnostic> = Vec::new();

        // Extract each section once.
        let (entities, e_diags) = self.entities();
        diagnostics.extend(e_diags);

        let (relations, r_diags) = self.relations();
        diagnostics.extend(r_diags);

        // Validate relation required entities against known entities.
        let known_eids: std::collections::BTreeSet<&str> = entities.iter().map(|e| e.id.as_str()).collect();
        for rel in &relations {
            for st in &rel.states {
                for eid in &st.required_entities {
                    if !known_eids.contains(eid.as_str()) {
                        diagnostics.push(ModelBrowseDiagnostic {
                            level: ModelBrowseDiagnosticLevel::Warning,
                            message: format!("Relation \"{}\" state \"{}\" requires unknown entity \"{eid}\"", rel.id, st.state,),
                            path: Some(format!("relations.{}.{}", rel.id, st.state)),
                        });
                    }
                }
            }
        }

        // Build entrypoints from already-extracted entities/relations
        // to avoid re-extraction and duplicate diagnostics.
        let (entrypoints, ep_diags) = self.build_entrypoints(&entities, &relations);
        diagnostics.extend(ep_diags);

        let (actions, a_diags) = self.actions();
        diagnostics.extend(a_diags);

        let states = self.states();

        // Deduplicate: keep only the first occurrence of each (level, message, path) tuple.
        let mut seen: std::collections::BTreeSet<(String, String, String)> = std::collections::BTreeSet::new();
        diagnostics.retain(|d| {
            let key = (format!("{:?}", d.level), d.message.clone(), d.path.clone().unwrap_or_default());
            seen.insert(key)
        });

        Ok(BrowsedModel { metadata: self.metadata(), entities, relations, entrypoints, actions, states, diagnostics })
    }

    /// Build entrypoints from already-extracted entities and relations,
    /// without re-calling `self.entities()` or `self.relations()`.
    fn build_entrypoints(&self, entities: &[BrowsedEntity], relations: &[BrowsedRelation]) -> (Vec<BrowsedEntrypoint>, Vec<ModelBrowseDiagnostic>) {
        let mut entrypoints: Vec<BrowsedEntrypoint> = Vec::new();
        let mut diagnostics: Vec<ModelBrowseDiagnostic> = Vec::new();

        // --- Checkbook labels ---
        if let Some(section) = self.spec.top(DSL_IDX_CHECKBOOK)
            && let Some(mapping) = section.as_mapping()
        {
            for (label, rel_ids_val) in mapping {
                let label = label.as_str().unwrap_or("").to_string();
                if label.is_empty() {
                    continue;
                }

                let relation_ids: Vec<String> =
                    rel_ids_val.as_sequence().map(|seq| seq.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();

                if relation_ids.is_empty() {
                    continue;
                }

                // Emit diagnostic for any referenced relation that doesn't exist.
                for rid in &relation_ids {
                    if !relations.iter().any(|r| r.id == *rid) {
                        diagnostics.push(ModelBrowseDiagnostic {
                            level: ModelBrowseDiagnosticLevel::Warning,
                            message: format!("Checkbook label \"{label}\" references missing relation \"{rid}\""),
                            path: Some(format!("checkbook.{label}")),
                        });
                    }
                }

                // Collect first-level entity IDs from reachable relations.
                let mut entity_ids: Vec<String> = Vec::new();
                for rel in relations {
                    if relation_ids.contains(&rel.id) {
                        for st in &rel.states {
                            for eid in &st.required_entities {
                                if !entity_ids.contains(eid) {
                                    entity_ids.push(eid.clone());
                                }
                            }
                        }
                    }
                }

                entrypoints.push(BrowsedEntrypoint::CheckbookLabel { label, relation_ids, entity_ids });
            }
        }

        // --- Bare entities as entrypoints ---
        for entity in entities {
            entrypoints.push(BrowsedEntrypoint::Entity { id: entity.id.clone(), descr: entity.descr.clone() });
        }

        (entrypoints, diagnostics)
    }
}

/// Convert a serde_yaml Value to a human-readable display string.
fn value_to_display(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::Null => "null".to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::String(s) => s.clone(),
        other => serde_yaml::to_string(other).unwrap_or_else(|_| format!("{other:?}")),
    }
}

/// Return a human-readable name for a YAML value kind.
fn value_kind_name(v: &serde_yaml::Value) -> &'static str {
    match v {
        serde_yaml::Value::Null => "null",
        serde_yaml::Value::Bool(_) => "bool",
        serde_yaml::Value::Number(_) => "number",
        serde_yaml::Value::String(_) => "string",
        serde_yaml::Value::Sequence(_) => "sequence",
        serde_yaml::Value::Mapping(_) => "mapping",
        serde_yaml::Value::Tagged(_) => "tagged value",
    }
}
