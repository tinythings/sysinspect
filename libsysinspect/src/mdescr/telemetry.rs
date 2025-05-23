use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetrySpec {
    minion: Option<Vec<EventSelector>>,
    action: Option<Vec<EventSelector>>,
    cycle: Option<Vec<EventSelector>>,
}

impl TelemetrySpec {
    /// Get the model telemetry spec
    pub fn minion(&self) -> Vec<EventSelector> {
        if let Some(m) = &self.minion { m.clone() } else { vec![] }
    }

    /// Get the action telemetry spec
    pub fn action(&self) -> Vec<EventSelector> {
        if let Some(m) = &self.action { m.clone() } else { vec![] }
    }

    /// Get the cycle telemetry spec
    pub fn cycle(&self) -> Vec<EventSelector> {
        if let Some(m) = &self.cycle { m.clone() } else { vec![] }
    }
}

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DataExportType {
    Model,
    Cycle,
    Action,
}

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum StaticDataDestination {
    Attribute,
    Body,
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

    #[serde(rename = "event-type")]
    event_type: Option<String>,

    #[serde(rename = "static-destination")]
    static_destination: Option<String>,

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

    /// Get the static destination
    pub fn static_destination(&self) -> StaticDataDestination {
        if self.static_destination.clone().unwrap_or_default().to_lowercase().eq("body") {
            return StaticDataDestination::Body;
        }

        StaticDataDestination::Attribute
    }

    /// Get the event type. Default is "cycle".
    pub fn event_type(&self) -> DataExportType {
        if let Some(t) = &self.event_type {
            match t.as_str() {
                "model" => DataExportType::Model,
                "action" => DataExportType::Action,
                _ => DataExportType::Cycle,
            }
        } else {
            DataExportType::Cycle
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataFilter {
    entity: Option<String>,
    actions: Option<Vec<String>>,
}

impl DataFilter {
    /// Get the event type
    pub fn entity(&self) -> String {
        if let Some(e) = &self.entity { e.clone() } else { "".to_string() }
    }

    /// Get the action list
    pub fn actions(&self) -> Vec<String> {
        if let Some(a) = &self.actions { a.clone() } else { vec![] }
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

    #[serde(rename = "use-map")]
    use_map: Option<bool>,

    export: DataExport,
    filter: Option<DataFilter>,
}

impl EventSelector {
    pub fn is_model_event(&self) -> bool {
        self.map.is_some() && self.reduce.is_some()
    }
    /// Get the select list
    pub fn select(&self) -> Vec<String> {
        if let Some(s) = &self.select { s.clone() } else { vec![] }
    }

    /// Get the data map specification
    pub fn dataspec(&self) -> IndexMap<String, String> {
        let mut out: IndexMap<String, String> = IndexMap::new();
        for (k, v) in &self.data {
            let s = serde_yaml::to_string(v).unwrap_or_default();
            if s.is_empty() {
                continue;
            }
            out.insert(k.clone(), s.trim().to_string());
        }
        out
    }

    /// Get the filter
    pub fn filter(&self) -> DataFilter {
        if let Some(f) = &self.filter { f.clone() } else { DataFilter::default() }
    }

    /// Get the configuration usage of the map/reduce functions
    pub fn use_map(&self) -> bool {
        if let Some(u) = &self.use_map { *u } else { false }
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
