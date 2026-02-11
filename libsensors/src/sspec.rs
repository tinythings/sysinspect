use indexmap::IndexMap;
use libcommon::SysinspectError;
use serde::Deserialize;
use serde_yaml::Value as YamlValue;
use std::str::FromStr;

/// Represents the sensor specification configuration.
///
/// Contains optional interval constraints and a collection of sensor configurations.
#[derive(Debug, Deserialize)]
pub struct SensorSpec {
    /// Optional interval range specification for sensors.
    #[serde(default)]
    interval: Option<IntervalRange>,

    /// Flattened map of sensor configurations indexed by their names.
    #[serde(flatten)]
    items: IndexMap<String, SensorConf>,
}

impl SensorSpec {
    /// Returns a reference to the optional interval range.
    ///
    /// # Returns
    /// An `Option` containing a reference to the `IntervalRange` if configured, or `None`.
    pub fn interval(&self) -> Option<&IntervalRange> {
        self.interval.as_ref()
    }

    /// Returns a reference to the sensor configuration map.
    ///
    /// # Returns
    /// A reference to the `IndexMap` containing all sensor configurations.
    pub fn items(&self) -> &IndexMap<String, SensorConf> {
        &self.items
    }

    /// Retrieves a sensor configuration by name.
    ///
    /// # Arguments
    /// * `name` - The name of the sensor to retrieve.
    ///
    /// # Returns
    /// An `Option` containing a reference to the `SensorConf` if found, or `None`.
    pub fn get(&self, name: &str) -> Option<&SensorConf> {
        self.items.get(name)
    }
}

/// Represents an interval range with minimum, maximum, and unit.
#[derive(Debug, Deserialize, Clone)]
pub struct IntervalRange {
    /// Minimum interval value.
    pub min: u64,
    /// Maximum interval value.
    pub max: u64,
    /// Unit of the interval (e.g., "seconds", "milliseconds").
    pub unit: String,
}

/// Represents the configuration for a single sensor.
#[derive(Debug, Deserialize, Clone)]
pub struct SensorConf {
    /// Optional list of profiles this sensor belongs to.
    #[serde(default)]
    profile: Option<Vec<String>>,

    /// Optional human-readable description of the sensor.
    #[serde(default)]
    description: Option<String>,

    /// The listener type/name for this sensor.
    listener: String,

    /// Optional command-line options for the sensor.
    #[serde(default)]
    opts: Vec<String>,

    /// Optional YAML arguments for sensor configuration.
    #[serde(default)]
    args: YamlValue,

    /// Optional event type associated with this sensor.
    #[serde(default)]
    event: Option<String>,
}

impl SensorConf {
    /// Returns the list of profiles for this sensor.
    ///
    /// Returns the default profile ["default"] if no profiles are configured.
    /// All profile names are converted to lowercase.
    ///
    /// # Returns
    /// A vector of lowercase profile names.
    pub fn profile(&self) -> Vec<String> {
        self.profile.clone().unwrap_or_else(|| vec!["default".to_string()]).into_iter().map(|p| p.to_lowercase()).collect()
    }

    /// Checks if this sensor matches any of the provided profiles.
    ///
    /// # Arguments
    /// * `profiles` - A slice of profile names to match against.
    ///
    /// # Returns
    /// `true` if any of this sensor's profiles matches any provided profile (case-insensitive), `false` otherwise.
    pub fn matches_profile(&self, profiles: &[String]) -> bool {
        self.profile().iter().any(|tag| profiles.iter().any(|m| m.eq_ignore_ascii_case(tag)))
    }

    /// Returns the description of this sensor.
    ///
    /// # Returns
    /// An `Option` containing a string reference if description is set, or `None`.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns the listener name for this sensor.
    ///
    /// # Returns
    /// A string reference to the listener name.
    pub fn listener(&self) -> &str {
        &self.listener
    }

    /// Returns the command-line options for this sensor.
    ///
    /// # Returns
    /// A slice of strings representing the options.
    pub fn opts(&self) -> &[String] {
        &self.opts
    }

    /// Returns the YAML arguments for this sensor.
    ///
    /// # Returns
    /// A reference to the `YamlValue` containing sensor arguments.
    pub fn args(&self) -> &YamlValue {
        &self.args
    }

    /// Returns the event type associated with this sensor.
    ///
    /// # Returns
    /// An `Option` containing a string reference if event is set, or `None`.
    pub fn event(&self) -> Option<&str> {
        self.event.as_deref()
    }
}

/// Implements parsing of `SensorSpec` from YAML string format.
impl FromStr for SensorSpec {
    type Err = SysinspectError;

    /// Parses a YAML string into a `SensorSpec`.
    ///
    /// Expects the YAML to have a top-level "sensors" key containing the sensor specification.
    ///
    /// # Arguments
    /// * `s` - A YAML-formatted string containing the sensor specification.
    ///
    /// # Returns
    /// A `Result` containing the parsed `SensorSpec` or a `SysinspectError` if parsing fails.
    fn from_str(s: &str) -> Result<Self, SysinspectError> {
        #[derive(Deserialize)]
        struct Wrapper {
            sensors: SensorSpec,
        }

        Ok(serde_yaml::from_str::<Wrapper>(s)?.sensors)
    }
}
