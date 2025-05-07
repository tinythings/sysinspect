use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetrySpec {
    model: Option<Vec<EventSelector>>,
    minion: Option<Vec<EventSelector>>,
}

impl TelemetrySpec {
    /// Get the model telemetry spec
    pub fn model(&self) -> Vec<EventSelector> {
        if let Some(m) = &self.model { m.clone() } else { vec![] }
    }

    /// Get the minion telemetry spec
    pub fn minion(&self) -> Vec<EventSelector> {
        if let Some(m) = &self.minion { m.clone() } else { vec![] }
    }
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
        self.attr_format.clone()
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

impl EventSelector {
    pub fn is_model_event(&self) -> bool {
        self.map.is_some() && self.reduce.is_some()
    }
    /// Get the select list
    pub fn select(&self) -> Vec<String> {
        if let Some(s) = &self.select { s.clone() } else { vec![] }
    }

    /// Get the data map
    pub fn data(&self) -> IndexMap<String, Value> {
        self.data.clone()
    }

    /// Get the map
    pub fn map(&self) -> IndexMap<String, String> {
        if let Some(m) = &self.map { m.clone() } else { IndexMap::new() }
    }

    /// Get the reduce
    pub fn reduce(&self) -> IndexMap<String, String> {
        if let Some(r) = &self.reduce { r.clone() } else { IndexMap::new() }
    }

    /// Get the export spec
    pub fn export(&self) -> DataExport {
        self.export.clone()
    }
}
