pub mod argparse;
pub mod bridge;
pub mod sensors;
pub mod service;
pub mod sspec;

#[cfg(test)]
mod lib_ut;

use crate::sspec::{IntervalRange, SensorConf, SensorSpec};
use colored::Colorize;
use indexmap::IndexMap;
use libcommon::SysinspectError;
use serde::Deserialize;
use std::{
    collections::HashSet,
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

fn path_depth(p: &Path) -> usize {
    p.components().count()
}

fn list_cfg_files(p: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(p)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("cfg"))
        .map(|e| e.into_path())
        .collect();
    files.sort();
    files
}

/// Collect cfg chunks in deterministic order with scope roots.
///
/// Scope root is a directory that contains `sensors.cfg`.
/// Rules:
/// - sibling scopes are allowed
/// - nested `sensors.cfg` is ignored (warn)
/// - each scope loads `sensors.cfg` first, then all other `*.cfg` recursively
/// - files outside any scope are ignored when at least one scope exists (warn)
fn collect_chunks(p: &Path) -> Vec<PathBuf> {
    let all = list_cfg_files(p);
    let mut idxs: Vec<PathBuf> = all.iter().filter(|x| x.file_name().and_then(|s| s.to_str()) == Some("sensors.cfg")).cloned().collect();

    // Strict mode: no sensors.cfg index => no chunks are loaded.
    if idxs.is_empty() {
        log::warn!(
            "No {} index found under {}. No sensor chunks will be loaded.",
            "\"sensors.cfg\"".yellow(),
            format!("\"{}\"", p.display()).yellow()
        );
        return vec![];
    }

    idxs.sort_by(|a, b| path_depth(a).cmp(&path_depth(b)).then(a.cmp(b)));

    let mut roots: Vec<PathBuf> = Vec::new();
    for idx in idxs {
        let dir = idx.parent().unwrap_or(p).to_path_buf();
        if let Some(parent_root) = roots.iter().find(|r| dir.starts_with(r.as_path())) {
            log::warn!("Ignoring nested sensors.cfg at {} (scope root is {})", idx.display(), parent_root.display());
            continue;
        }
        roots.push(dir);
    }
    roots.sort();

    let mut chunks = Vec::<PathBuf>::new();
    let mut included = HashSet::<PathBuf>::new();

    for root in &roots {
        let idx = root.join("sensors.cfg");
        if idx.exists() {
            chunks.push(idx.clone());
            included.insert(idx);
        }

        let mut sub = list_cfg_files(root);
        sub.retain(|f| f.file_name().and_then(|s| s.to_str()) != Some("sensors.cfg"));
        for f in sub {
            if included.insert(f.clone()) {
                chunks.push(f);
            }
        }
    }

    for f in &all {
        if f.file_name().and_then(|s| s.to_str()) == Some("sensors.cfg") {
            continue;
        }
        if !included.contains(f) {
            log::warn!("Ignoring cfg outside any sensors scope (missing sibling sensors.cfg): {}", f.display());
        }
    }

    chunks
}

fn merged_events_yaml(p: &Path) -> Result<serde_yaml::Value, SysinspectError> {
    let mut events_map: serde_yaml::Mapping = serde_yaml::Mapping::new();
    for path in collect_chunks(p) {
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

    let mut interval: Option<IntervalRange> = None;
    let mut sensors: IndexMap<String, SensorConf> = IndexMap::new();
    let mut events_map: serde_yaml::Mapping = serde_yaml::Mapping::new();

    for path in collect_chunks(p) {
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

            // interval rule: first defined wins, all others are ignored.
            if interval.is_none() {
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
