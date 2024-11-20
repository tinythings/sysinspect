use std::sync::{Arc, Mutex};

use crate::SysinspectError;

/// Targeting schemes
pub static SCHEME_MODEL: &str = "model://";
pub static SCHEME_STATE: &str = "state://";

///
/// Query parser (scheme).
/// It has the following format:
///
///     <model>/[entity]/[state]
///     <model>:[checkbook labels]
///
/// If `"entity"` and/or `"state"` are omitted, they are globbed to `"$"` (all).
#[derive(Debug, Clone, Default)]
pub struct MinionQuery {
    src: String,
    entity: Option<String>,
    state: Option<String>,
    scheme: String,
    labels: Option<String>,
}

impl MinionQuery {
    pub fn new(q: &str) -> Result<Arc<Mutex<Self>>, SysinspectError> {
        let q = q.trim();
        if !q.starts_with(SCHEME_STATE) && !q.starts_with(SCHEME_MODEL) {
            return Err(SysinspectError::ProtoError("Query has unknown scheme".to_string()));
        }

        let sq: Vec<&str> = q.split("://").collect();
        if sq.len() != 2 {
            return Err(SysinspectError::ProtoError("Unable to parse scheme".to_string()));
        }

        let mut instance = Self { ..Default::default() };
        instance.scheme = sq[0].to_owned();

        let precise = sq[1].contains('/');
        let sq: Vec<&str> = sq[1].split(if precise { '/' } else { ':' }).filter(|s| !s.is_empty()).collect();
        match sq.len() {
            0 => {
                return Err(SysinspectError::ProtoError("No model has been targeted".to_string()));
            }
            1 => instance.src = sq[0].to_string(),
            2 => {
                instance.src = sq[0].to_string();
                if precise {
                    instance.entity = Some(sq[1].to_string());
                } else {
                    instance.labels = Some(sq[1].to_string());
                }
            }
            3 => {
                instance.src = sq[0].to_string();
                if precise {
                    instance.entity = Some(sq[1].to_string());
                    instance.state = Some(sq[2].to_string());
                }
            }
            _ => {}
        }

        Ok(Arc::new(Mutex::new(instance)))
    }

    /// Get target model name
    pub fn target(&self) -> &str {
        &self.src
    }

    /// Get entities, comma-separated
    pub fn entities(&self) -> Vec<String> {
        if let Some(entity) = &self.entity {
            return entity.split(',').map(|s| s.to_string()).collect();
        }

        vec![]
    }

    /// Get checkbook labels, comma-separated
    pub fn checkbook_labels(&self) -> Vec<String> {
        if let Some(l) = &self.labels {
            return l.split(',').map(|s| s.to_string()).collect();
        }
        vec![]
    }

    /// Get desired state of the model
    pub fn state(&self) -> Option<String> {
        if let Some(state) = &self.state {
            return Some(state.to_owned());
        }

        None
    }
}