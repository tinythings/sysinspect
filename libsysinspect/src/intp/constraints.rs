use crate::SysinspectError;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Expr {
    any: Option<Vec<String>>,
    all: Option<Vec<String>>,

    #[serde(rename = "has-fact")]
    has_fact: Option<Vec<String>>,
}

impl Expr {
    /// Logical `OR` expression
    pub fn any(&self) -> Vec<String> {
        self.any.to_owned().unwrap_or_default()
    }

    /// Logical `AND` expression
    pub fn all(&self) -> Vec<String> {
        self.all.to_owned().unwrap_or_default()
    }

    /// Returns a list of fact IDs. This is a condition
    /// that filtering-out all entities that do **not** have **any** of these facts.
    pub fn has_fact(&self) -> Vec<String> {
        self.has_fact.to_owned().unwrap_or_default()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Constraint {
    id: Option<String>,
    descr: Option<String>,

    #[serde(rename = "expr")]
    entities: HashMap<String, Expr>,
}

impl Constraint {
    pub fn new(id: &Value, constraint: &Value) -> Result<Self, SysinspectError> {
        let mut instance = Constraint::default();
        let c_id: String;

        if let Some(id) = id.as_str() {
            c_id = id.to_string();
        } else {
            return Err(SysinspectError::ModelDSLError("Constraint does not have an ID assigned".to_string()));
        }

        if let Ok(mut c) = serde_yaml::from_value::<Constraint>(constraint.to_owned()) {
            c.id = Some(c_id);
            instance = c;
        }

        Ok(instance)
    }

    /// Get `id` of the Constraint
    pub fn id(&self) -> String {
        self.id.to_owned().unwrap_or("".to_string())
    }

    /// Get `description` of the Constraint.
    /// Field is **optional**.
    pub fn descr(&self) -> String {
        self.descr.to_owned().unwrap_or("".to_string())
    }
}
