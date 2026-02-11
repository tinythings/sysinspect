pub mod sensors;
pub mod sspec;

use crate::sspec::{IntervalRange, SensorConf, SensorSpec};
use indexmap::IndexMap;
use libcommon::SysinspectError;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Deserialize)]
struct Wrapper {
    sensors: Option<SensorSpec>,
}

/// Loads sensor specifications from configuration files in the given directory.
/// The function searches for `.cfg` files, parses them as YAML, and merges their
/// contents into a single `SensorSpec` instance. The first defined interval and
/// sensor configurations take precedence in case of duplicates.
///
/// Merge policy: first wins for both interval and sensor items.
///
/// # Arguments
/// * `path` - A reference to the directory path containing sensor configuration files.
///
/// # Returns
/// A `Result` containing the merged `SensorSpec` if successful, or a `SysinspectError` if an error occurs during file reading or parsing.
///
/// # Errors
/// * `SysinspectError` - If there is an error reading files or parsing YAML content.
///
/// # Example
/// ```
/// use std::path::Path;
/// use libsensors::load;
/// let spec = load(Path::new("/path/to/sensor/configs")).expect("Failed to load sensor specifications");
/// println!("{:#?}", spec);
/// ```
pub fn load(p: &Path) -> Result<SensorSpec, SysinspectError> {
    let mut interval: Option<IntervalRange> = None;
    let mut sensors: IndexMap<String, SensorConf> = IndexMap::new();

    let mut chunks: Vec<_> = WalkDir::new(p)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("cfg"))
        .map(|e| e.into_path())
        .collect();

    chunks.sort();

    for path in chunks {
        let w: Wrapper = match serde_yaml::from_str(&fs::read_to_string(&path)?) {
            Ok(p) => p,
            Err(err) => {
                log::warn!("Skipping invalid DSL file {}: {}", path.display(), err);
                continue;
            }
        };

        let Some(spec) = w.sensors else {
            continue;
        };

        if interval.is_none() {
            interval = spec.interval().cloned();
        } else if spec.interval().is_some() {
            log::warn!("Interval already defined. Ignoring interval in {}", path.display());
        }

        for (k, v) in spec.items() {
            if sensors.contains_key(k) {
                log::warn!("Duplicate sensor '{}' in {} ignored (first wins)", k, path.display());
                continue;
            }
            sensors.insert(k.clone(), v.clone());
        }
    }

    // Sort all sensors alphabetically
    let mut sorted: Vec<_> = sensors.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut sensors = IndexMap::new();
    for (k, v) in sorted {
        sensors.insert(k, v);
    }

    Ok(SensorSpec::new(interval, sensors))
}
