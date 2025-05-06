use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetrySpec {
    model: Option<Vec<EventSelector>>,
    minion: Option<Vec<EventSelector>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataExport {
    // Name of the attribute where the data is stored in the OTEL's JSON container
    #[serde(rename = "attr-name")]
    attr_name: String,

    // The very type of the attribute. Usually always a string.
    #[serde(rename = "attr-type")]
    attr_type: Option<String>,

    // The format template of the attribute.
    #[serde(rename = "attr-format")]
    attr_format: Option<String>,

    #[serde(rename = "data-type")]
    data_type: Option<IndexMap<String, String>>,

    #[serde(rename = "telemetry-type")]
    telemetry_type: Option<String>,

    #[serde(rename = "static")]
    static_data: Option<IndexMap<String, Value>>,
}

impl DataExport {
    /// Get the OTEL message attribute name
    pub fn attr_name(&self) -> String {
        self.attr_name.clone()
    }

    /// Get the attribute format
    /// If not set, return the default value which is "json"
    pub fn attr_type(&self) -> String {
        if let Some(t) = &self.attr_type { t.clone() } else { "json".to_string() }
    }

    /// Return the attribute format.
    pub fn attr_format(&self) -> Option<String> {
        if let Some(f) = &self.attr_format { Some(f.clone()) } else { None }
    }

    /// Get the explicit data type cast mapping
    pub fn cast_map(&self) -> IndexMap<String, String> {
        if let Some(t) = &self.data_type { t.clone() } else { IndexMap::new() }
    }

    /// Get the telemetry type. Default is "log".
    pub fn telemetry_type(&self) -> String {
        if let Some(t) = &self.telemetry_type { t.clone() } else { "log".to_string() }
    }

    /// Get the static data
    pub fn static_data(&self) -> IndexMap<String, Value> {
        if let Some(s) = &self.static_data { s.clone() } else { IndexMap::new() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventSelector {
    select: Option<Vec<String>>, // Or ["*"]
    data: IndexMap<String, Value>,

    // Map: key of data to a function expression
    map: Option<IndexMap<String, String>>,

    // Reduce: reduce with key of data and a function expression
    reduce: Option<IndexMap<String, String>>,

    export: DataExport,
}
