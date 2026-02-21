pub mod argparse;
pub mod sensors;
pub mod service;
pub mod sspec;

use crate::sspec::{IntervalRange, SensorConf, SensorSpec};
use indexmap::IndexMap;
use libcommon::SysinspectError;
use serde::Deserialize;
use std::{fs, path::Path};
use walkdir::WalkDir;

#[derive(Deserialize)]
struct Wrapper {
    sensors: Option<SensorSpec>,
    #[serde(default)]
    events: Option<serde_yaml::Value>,
}

pub fn load(p: &Path) -> Result<SensorSpec, SysinspectError> {
    log::info!("Loading sensor specifications from {}", p.display());

    let mut interval: Option<IntervalRange> = None;
    let mut sensors: IndexMap<String, SensorConf> = IndexMap::new();
    let mut events: Option<serde_yaml::Value> = None;

    let mut chunks: Vec<_> = WalkDir::new(p)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("cfg"))
        .map(|e| e.into_path())
        .collect();

    chunks.sort();

    for path in chunks {
        log::debug!("Loading sensors chunk: {}", path.display());
        let w: Wrapper = match serde_yaml::from_str(&fs::read_to_string(&path)?) {
            Ok(p) => p,
            Err(err) => {
                log::warn!("Skipping invalid DSL file {}: {}", path.display(), err);
                continue;
            }
        };

        let Some(mut spec) = w.sensors else {
            continue;
        };

        // first interval wins
        if interval.is_none() {
            interval = spec.interval_range().cloned();
        } else if spec.interval_range().is_some() {
            log::warn!("Interval already defined. Ignoring interval in {}", path.display());
        }

        // first sensor wins
        for (k, v) in spec.items() {
            if sensors.contains_key(&k) {
                log::warn!("Duplicate sensor '{}' in {} ignored (first wins)", k, path.display());
                continue;
            }
            sensors.insert(k.clone(), v.clone());
        }

        // first events block wins (same rule as interval)
        if events.is_none() {
            events = w.events.clone();
        } else if w.events.is_some() {
            log::warn!("Events already defined. Ignoring events in {}", path.display());
        }
    }

    // Sort sensors alphabetically
    let mut sorted: Vec<_> = sensors.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut sensors = IndexMap::new();
    for (k, v) in sorted {
        sensors.insert(k, v);
    }

    let mut out = SensorSpec::new(interval, sensors);

    if let Some(ev) = events {
        out.set_events_yaml(ev)?; // we’ll add this setter in sspec.rs
    }

    Ok(out)
}
