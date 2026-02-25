pub mod argparse;
pub mod bridge;
pub mod sensors;
pub mod service;
pub mod sspec;

#[cfg(test)]
mod lib_ut;

use crate::sspec::{IntervalRange, SensorConf, SensorSpec};
use indexmap::IndexMap;
use libcommon::SysinspectError;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Deserialize, Default)]
struct Wrapper {
    #[serde(default)]
    sensors: Option<SensorSpec>,
    #[serde(default)]
    events: Option<serde_yaml::Value>,
}

fn merged_events_yaml(p: &Path) -> Result<serde_yaml::Value, SysinspectError> {
    let root_cfg: PathBuf = p.join("sensors.cfg");
    let mut events_map: serde_yaml::Mapping = serde_yaml::Mapping::new();

    let mut chunks: Vec<PathBuf> = WalkDir::new(p)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("cfg"))
        .map(|e| e.into_path())
        .collect();

    chunks.sort();

    // root sensors.cfg first (if exists)
    if root_cfg.exists() {
        chunks.retain(|x| x != &root_cfg);
        chunks.insert(0, root_cfg.clone());
    }

    for path in chunks {
        // ignore nested sensors.cfg
        if path.file_name().and_then(|s| s.to_str()) == Some("sensors.cfg") && path != root_cfg {
            continue;
        }

        let mut w: Wrapper = match serde_yaml::from_str(&fs::read_to_string(&path)?) {
            Ok(v) => v,
            Err(err) => {
                log::warn!("Skipping invalid DSL file {}: {}", path.display(), err);
                continue;
            }
        };

        if let Some(ev) = w.events.take() {
            merge_events(&mut events_map, ev, &path);
        }
    }

    Ok(serde_yaml::Value::Mapping(events_map))
}

fn merge_sensors(dst: &mut IndexMap<String, SensorConf>, src: &SensorSpec, path: &Path) {
    for (k, v) in src.items_raw() {
        if dst.contains_key(k) {
            log::warn!("Duplicate sensor '{}' in {} ignored (first wins)", k, path.display());
            continue;
        }
        dst.insert(k.clone(), v.clone());
    }
}

fn merge_events(dst: &mut serde_yaml::Mapping, src: serde_yaml::Value, path: &Path) {
    let Some(src_map) = src.as_mapping() else {
        log::warn!("Events in {} ignored (expected mapping)", path.display());
        return;
    };

    for (k, v) in src_map {
        if dst.contains_key(k) {
            log::warn!("Duplicate event {:?} in {} ignored (first wins)", k, path.display());
            continue;
        }
        dst.insert(k.clone(), v.clone());
    }
}

pub fn load(p: &Path) -> Result<SensorSpec, SysinspectError> {
    log::info!("Loading sensor specifications from {}", p.display());

    let root_cfg: PathBuf = p.join("sensors.cfg");

    let mut interval: Option<IntervalRange> = None;
    let mut sensors: IndexMap<String, SensorConf> = IndexMap::new();
    let mut events_map: serde_yaml::Mapping = serde_yaml::Mapping::new();

    let mut chunks: Vec<PathBuf> = WalkDir::new(p)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("cfg"))
        .map(|e| e.into_path())
        .collect();

    chunks.sort();

    // Enforce: only $ROOT/sensors/sensors.cfg is allowed as sensors.cfg
    for path in &chunks {
        if path.file_name().and_then(|s| s.to_str()) == Some("sensors.cfg") && path != &root_cfg {
            log::warn!("Ignoring nested sensors.cfg at {} (only {} is allowed)", path.display(), root_cfg.display());
        }
    }

    // Process root sensors.cfg first (if it exists)
    if root_cfg.exists() {
        chunks.retain(|x| x != &root_cfg);
        chunks.insert(0, root_cfg.clone());
    }

    for path in chunks {
        // skip forbidden nested sensors.cfg
        if path.file_name().and_then(|s| s.to_str()) == Some("sensors.cfg") && path != root_cfg {
            continue;
        }

        log::debug!("Loading sensors chunk: {}", path.display());
        let w: Wrapper = match serde_yaml::from_str(&fs::read_to_string(&path)?) {
            Ok(v) => v,
            Err(err) => {
                log::warn!("Skipping invalid DSL file {}: {}", path.display(), err);
                continue;
            }
        };

        // sensors + interval
        if let Some(spec) = w.sensors.as_ref() {
            let this_interval = spec.interval_range().cloned();

            // interval rule:
            // - if interval defined in root sensors.cfg => it wins, ignore all others
            // - otherwise first interval wins
            if path == root_cfg {
                if this_interval.is_some() {
                    interval = this_interval;
                }
            } else if interval.is_none() {
                interval = this_interval;
            } else if this_interval.is_some() {
                log::warn!("Interval already defined. Ignoring interval in {}", path.display());
            }

            merge_sensors(&mut sensors, spec, &path);
        }

        // events (merged, first key wins)
        if let Some(ev) = w.events {
            merge_events(&mut events_map, ev, &path);
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

    if !events_map.is_empty() {
        out.set_events_yaml(serde_yaml::Value::Mapping(events_map))?;
    }

    let _ = out.items(); // apply global interval to missing per-sensor intervals

    Ok(out)
}
