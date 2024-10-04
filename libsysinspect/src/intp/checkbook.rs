use super::relations::Relation;
use serde_yaml::Value;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct CheckbookSection {
    relations: HashMap<String, Vec<Relation>>,
}

impl CheckbookSection {
    /// Initialise a checkbook.
    /// Entry is a list of relations needs to be examined.
    pub fn new(label: &Value, rel_ids: &Value, relations: &HashMap<String, Relation>) -> Option<Self> {
        let mut instance = CheckbookSection::default();
        let label = label.as_str().unwrap_or("");

        if let Some(rel_ids) = rel_ids.as_sequence() {
            for rid in rel_ids.iter().filter_map(|s| s.as_str()).filter(|s| !s.is_empty()) {
                if let Some(rel) = relations.get(rid) {
                    instance.add_relation(label.to_string(), rel.to_owned());
                } else {
                    log::warn!("Checkbook section \"{label}\" cannot find relation \"{rid}\"");
                }
            }
        }

        if instance.relations.is_empty() {
            return None;
        }

        Some(instance)
    }

    /// Add a relation to the checkbook mapping
    fn add_relation(&mut self, id: String, rel: Relation) {
        self.relations.entry(id.to_owned()).or_default().push(rel);
    }
}
