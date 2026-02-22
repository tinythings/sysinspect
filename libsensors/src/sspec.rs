use indexmap::IndexMap;
use libcommon::SysinspectError;
use libsysinspect::intp::conf::EventsConfig;
use serde::Deserialize;
use serde_yaml::Value as YamlValue;
use std::{
    mem,
    str::FromStr,
    time::{Duration, SystemTime},
};

/// Global default interval range from `sensors.interval`.
#[derive(Debug, Deserialize, Clone)]
/// Represents a range of intervals with a unit.
///
/// This struct defines minimum and maximum bounds for time intervals and
/// provides methods to normalize and pick random values within the range.
pub struct IntervalRange {
    /// Minimum interval value
    pub min: u64,
    /// Maximum interval value
    pub max: u64,
    /// Unit of time (e.g., "seconds", "ms", "minutes")
    pub unit: String,
}

impl IntervalRange {
    /// Returns a tuple of (min, max) with min <= max and both >= 1.
    ///
    /// Normalizes the interval range by ensuring both values are at least 1
    /// and that min <= max. If the input has min > max, they are swapped.
    ///
    /// # Returns
    ///
    /// A tuple `(min, max)` where `min <= max` and both values are >= 1.
    ///
    /// # Examples
    ///
    /// ```
    /// let range = IntervalRange { min: 5, max: 10, unit: "seconds".to_string() };
    /// assert_eq!(range.range(), (5, 10));
    /// ```
    ///
    /// ```
    /// let range = IntervalRange { min: 10, max: 5, unit: "seconds".to_string() };
    /// assert_eq!(range.range(), (5, 10)); // swapped
    /// ```
    pub fn range(&self) -> (u64, u64) {
        let mut a = self.min.max(1);
        let mut b = self.max.max(1);
        if a > b {
            mem::swap(&mut a, &mut b);
        }
        (a, b)
    }

    /// Picks a random value within the interval range using the current system time as seed.
    ///
    /// This method normalizes the interval range and generates a pseudo-random value
    /// within that range using the system time in nanoseconds as the random seed.
    ///
    /// # Returns
    ///
    /// A random `u64` value within the normalized interval range (inclusive).
    ///
    /// # Examples
    ///
    /// ```
    /// let range = IntervalRange { min: 1, max: 10, unit: "seconds".to_string() };
    /// let value = range.pick();
    /// assert!(value >= 1 && value <= 10);
    /// ```
    pub fn pick(&self) -> u64 {
        let (min, max) = self.range();
        fastrand::u64(min..=max)
    }
}

#[derive(Debug, Deserialize)]
pub struct SensorSpec {
    #[serde(default)]
    interval: Option<IntervalRange>,

    #[serde(flatten)]
    items: IndexMap<String, SensorConf>,

    #[serde(skip)]
    updated: bool, // Marker that items were updated with the interval

    #[serde(skip)]
    events: Option<EventsConfig>, // EventConfig placeholder, added later
}

impl SensorSpec {
    pub fn new(interval: Option<IntervalRange>, items: IndexMap<String, SensorConf>) -> Self {
        SensorSpec { interval, items, updated: false, events: None }
    }

    pub fn items_raw(&self) -> &IndexMap<String, SensorConf> {
        &self.items
    }

    /// For loader merge (first wins).
    pub fn interval_range(&self) -> Option<&IntervalRange> {
        self.interval.as_ref()
    }

    fn pick_range(min: u64, max: u64) -> u64 {
        let mut a = min.max(1);
        let mut b = max.max(1);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        fastrand::seed(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos() as u64);
        fastrand::u64(a..=b)
    }

    fn global_range(&self) -> IntervalRange {
        self.interval.clone().unwrap_or(IntervalRange { min: 3, max: 10, unit: "seconds".to_string() })
    }

    fn u2d(v: u64, unit: &str) -> Duration {
        match unit.to_lowercase().as_str() {
            "ms" | "msec" | "millisecond" | "milliseconds" => Duration::from_millis(v),
            "s" | "sec" | "second" | "seconds" => Duration::from_secs(v),
            "m" | "min" | "minute" | "minutes" => Duration::from_secs(v.saturating_mul(60)),
            "h" | "hr" | "hour" | "hours" => Duration::from_secs(v.saturating_mul(60 * 60)),
            _ => Duration::from_secs(v),
        }
    }

    pub fn interval(&self) -> Duration {
        let range = self.global_range();
        let mut min = range.min.max(1);
        let mut max = range.max.max(1);
        if min > max {
            std::mem::swap(&mut min, &mut max);
        }
        Self::u2d(Self::pick_range(min, max), &range.unit)
    }

    /// Updates sensorconf with the interval, if not defined
    pub fn items(&mut self) -> IndexMap<String, SensorConf> {
        if self.updated {
            return self.items.clone();
        }

        let range = self.global_range();
        for (_, config) in self.items.iter_mut() {
            if config.interval().is_none() {
                let mut c = config.clone();
                c.interval = Some(Self::u2d(Self::pick_range(range.min, range.max), &range.unit));
                *config = c;
            }
        }
        self.updated = true;
        self.items.clone()
    }

    /// Get a sensor settings configuration by its name
    pub fn sensor_config(&self, name: &str) -> Option<&SensorConf> {
        self.items.get(name)
    }

    /// Get the entire events configuration, if defined. This is added later by the loader.
    pub fn events_config(&self) -> Option<&EventsConfig> {
        self.events.as_ref()
    }

    pub fn set_events_yaml(&mut self, ev: serde_yaml::Value) -> Result<(), SysinspectError> {
        let mut cfg = EventsConfig::default();
        cfg.set_events(&ev)?;
        self.events = Some(cfg);
        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SensorConf {
    #[serde(default)]
    profile: Option<Vec<String>>,

    #[serde(default)]
    description: Option<String>,

    listener: String,

    #[serde(default)]
    opts: Vec<String>,

    #[serde(default)]
    args: YamlValue,

    #[serde(default)]
    tag: Option<String>,

    #[serde(default)]
    interval: Option<Duration>,
}

impl SensorConf {
    /// Returns the profile list, defaulting to `["default"]` if not specified.
    ///
    /// All profile names are converted to lowercase for consistent comparison.
    pub fn profile(&self) -> Vec<String> {
        self.profile.clone().unwrap_or_else(|| vec!["default".to_string()]).into_iter().map(|p| p.to_lowercase()).collect()
    }

    /// Checks if any of this sensor's profiles match any in the provided list (case-insensitive).
    ///
    /// # Arguments
    ///
    /// * `profiles` - A slice of profile names to match against
    ///
    /// # Returns
    ///
    /// `true` if there is at least one matching profile, `false` otherwise.
    pub fn matches_profile(&self, profiles: &[String]) -> bool {
        self.profile().iter().any(|tag| profiles.iter().any(|m| m.eq_ignore_ascii_case(tag)))
    }

    /// Returns the sensor's description, if defined.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns the listener name for this sensor configuration.
    pub fn listener(&self) -> &str {
        &self.listener
    }

    /// Returns the command-line options for this sensor.
    pub fn opts(&self) -> &[String] {
        &self.opts
    }

    /// Returns the YAML arguments for this sensor.
    pub fn args(&self) -> &YamlValue {
        &self.args
    }

    /// Returns the tag associated with this sensor, if defined.
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    /// Returns the interval duration for this sensor, if defined.
    pub fn interval(&self) -> Option<Duration> {
        self.interval
    }
}

impl FromStr for SensorSpec {
    type Err = SysinspectError;

    fn from_str(s: &str) -> Result<Self, SysinspectError> {
        #[derive(Deserialize)]
        struct Wrapper {
            sensors: SensorSpec,
            #[serde(default)]
            events: Option<YamlValue>,
        }
        let w = serde_yaml::from_str::<Wrapper>(s)?;
        let mut spec = w.sensors;
        if let Some(ev) = w.events {
            let mut cfg = EventsConfig::default();
            cfg.set_events(&ev)?;
            spec.events = Some(cfg);
        }

        Ok(spec)
    }
}
