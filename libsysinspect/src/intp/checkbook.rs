use super::relations::Relation;
use serde_yaml::Value;
use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Default)]
pub struct CheckbookSection {
    id: String,
    relations: Vec<Relation>,
}

impl CheckbookSection {
    /// Initialise a checkbook.
    /// Entry is a list of relations needs to be examined.
    pub fn new(label: &Value, rel_ids: &Value, relations: &HashMap<String, Relation>) -> Option<Self> {
        let mut instance = CheckbookSection::default();

        // No relations defined anyway
        if relations.is_empty() {
            return None;
        }

        // Check if there is at least one requested Id in the set of relations
        let mut orphans: Vec<String> = Vec::default();
        if let Some(rel_ids) = rel_ids.as_sequence() {
            for rid in rel_ids.iter().map(|s| s.as_str().unwrap_or("").to_string()).collect::<Vec<String>>() {
                if let Some(rel) = relations.get(&rid) {
                    instance.relations.push(rel.to_owned());
                } else {
                    orphans.push(rid);
                }
            }
            instance.id = label.as_str().unwrap_or("").to_string();
        }

        // Checks
        if instance.id.is_empty() {
            log::error!("Checkbook section should have an Id");
            return None;
        }

        // Feedback only
        if !orphans.is_empty() {
            log::warn!("Checkbook section \"{}\" has {} bogus relations: {}", instance.id, orphans.len(), orphans.join(", "));
        }

        // Discard invalid section
        if instance.relations.is_empty() {
            log::error!("Checkbook \"{}\" has no valid relations assotiated", instance.id);
            return None;
        }

        Some(instance)
    }

    /// Get Id of the checkbook section
    pub fn id(&self) -> String {
        self.id.to_owned()
    }
}

impl Display for CheckbookSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<CheckbookSection - Id: {}, Relations: {:?}>", self.id, self.relations)?;
        Ok(())
    }
}
