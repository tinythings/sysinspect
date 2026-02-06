use super::{datapatch, mspecdef::ModelSpec};
use crate::{
    SysinspectError,
    cfg::mmconf::{MinionConfig, SysInspectConfig},
    tmpl::render::ModelTplRender,
    traits::systraits::SystemTraits,
};
use indexmap::IndexMap;
use serde_yaml::Value;
use std::{
    fs::{self},
    path::{Path, PathBuf},
    sync::Arc,
};
use walkdir::{DirEntry, WalkDir};

pub const MODEL_INDEX: &str = "model.cfg";
pub const MODEL_FILE_EXT: &str = ".cfg";

/// Spec loader object
struct SpecLoader {
    // Path to the model
    pth: PathBuf,

    // Only one init allowed
    init: bool,

    // System traits if running in distributed mode
    traits: Option<IndexMap<String, serde_json::Value>>,

    // Load existing context to the model for Jinja templates rendering and conditions
    context: Option<IndexMap<String, serde_json::Value>>,

    // Minion config
    cfg: Arc<MinionConfig>,
}

impl SpecLoader {
    // Constructor
    fn new(cfg: Arc<MinionConfig>, pth: PathBuf, traits: Option<SystemTraits>, context: Option<IndexMap<String, serde_json::Value>>) -> Self {
        let mut ext: Option<IndexMap<String, serde_json::Value>> = None;
        if let Some(traits) = traits {
            let mut et: IndexMap<String, serde_json::Value> = IndexMap::new();
            for k in traits.trait_keys() {
                if let Some(v) = traits.get(&k) {
                    et.insert(k, v);
                }
            }
            ext = Some(et);
        }

        Self { cfg, pth, init: false, traits: ext, context }
    }

    /// Collect YAML parts of the model from a different files
    fn collect_by_path(&mut self, p: &Path, inherit: bool) -> Result<Vec<Value>, SysinspectError> {
        let mut out = Vec::<Value>::new();

        for etr in WalkDir::new(p).follow_links(true).into_iter().filter_map(Result::ok) {
            // Skip dirs
            if !etr.path().is_file() {
                continue;
            }

            // Crash if multiple indexes found (or we are at multuple models)
            if let Some(fname) = etr.path().file_name().and_then(|s| s.to_str()) {
                if !fname.ends_with(MODEL_FILE_EXT) {
                    continue;
                }

                if fname == MODEL_INDEX && !inherit {
                    if self.init {
                        return Err(SysinspectError::ModelMultipleIndex(etr.path().as_os_str().to_str().unwrap().to_string()));
                    } else {
                        self.init = true;
                        out.insert(0, self.setup_tera_template(fname, &etr)?);
                    }
                } else if fname != MODEL_INDEX {
                    out.push(self.setup_tera_template(fname, &etr)?);
                } else {
                    log::debug!("Skipping inherited index");
                }
            }
        }

        Ok(out)
    }

    /// Setup Tera template for the given file with the built-in data variables (traits, context etc.)
    fn setup_tera_template(&mut self, fname: &str, etr: &DirEntry) -> Result<Value, SysinspectError> {
        //// Render the index file
        let mut mtr = ModelTplRender::new(fname, &fs::read_to_string(etr.path())?);
        if let Some(traits) = self.traits.clone() {
            mtr.set_ns_values("traits", traits);
            mtr.set_values("context", self.context.clone().unwrap_or_default());
        }

        match serde_yaml::from_str::<Value>(&mtr.render()?) {
            Ok(chunk) => Ok(chunk),
            Err(err) => Err(SysinspectError::ModelDSLError(format!("Unable to parse {fname}: {err}"))),
        }
    }

    /// Merge YAML parts
    fn merge_parts(&mut self, chunks: &mut Vec<Value>) -> Result<Value, SysinspectError> {
        if chunks.is_empty() {
            return Err(SysinspectError::ModelMultipleIndex("No data found".to_string()));
        }

        let mut base = chunks.remove(0);

        for chunk in chunks {
            match (&mut base, chunk) {
                (Value::Mapping(amp), Value::Mapping(bmp)) => {
                    for (k, v) in bmp {
                        if let Some(av) = amp.get_mut(k) {
                            if let (Some(av_map), Some(v_map)) = (av.as_mapping_mut(), v.as_mapping_mut()) {
                                v_map.extend(av_map.iter().map(|(k, v)| (k.clone(), v.clone())));
                            }
                            *av = v.clone();
                        } else {
                            amp.insert(k.clone(), v.clone());
                        }
                    }
                }
                (a, b) => {
                    // Non-null "b" implies a structure, which is not formed as a key/val,
                    // therefore cannot be added to the DSL root
                    if !b.is_null() {
                        return Err(SysinspectError::ModelDSLError(format!("Mapping expected, but this structure passed: {a:?}\n\t > {b:?}")));
                    }
                }
            }
        }
        Ok(base)
    }

    /// Load model spec by merging all the data parts and validating
    /// its content.
    fn load(&mut self) -> Result<ModelSpec, SysinspectError> {
        let mpt: Vec<Value> = match self.collect_by_path(&self.pth.to_owned(), false) {
            Ok(mpt) => mpt,
            Err(err) => {
                return Err(SysinspectError::ModelDSLError(format!(
                    "Unable to load model spec while collecting parts by path from {}: {err}",
                    self.pth.to_str().unwrap_or_default()
                )));
            }
        };

        let mut base: Vec<Value> = Vec::default();
        let mut iht: Vec<Value> = Vec::default();

        // Try inheriting
        if !mpt.is_empty() {
            let basempt: ModelSpec = match serde_yaml::from_value(mpt[0].to_owned()) {
                Ok(basempt) => basempt,
                Err(err) => {
                    return Err(SysinspectError::ModelDSLError(format!(
                        "Unable to load root model.cfg spec: {}: {err}",
                        self.pth.to_str().unwrap_or_default()
                    )));
                }
            };

            if let Some(ipth) = basempt.inherits() {
                let ipth = fs::canonicalize(self.pth.join(ipth))?; // Redirect path
                base.insert(0, mpt[0].to_owned());
                base.extend(match self.collect_by_path(&ipth, true) {
                    Ok(parts) => parts,
                    Err(err) => return Err(SysinspectError::ModelDSLError(format!("Unable to load inherited model spec: {err}"))),
                });
                iht.extend(mpt[1..].iter().map(|e| e.to_owned()).collect::<Vec<Value>>());
            } else {
                base.extend(mpt);
            }
        } else {
            return Err(SysinspectError::ModelDSLError(format!("No model found at {}", self.pth.to_str().unwrap_or_default())));
        }

        // Merge minion config to the main model
        base.push(SysInspectConfig::default().set_minion_config((*self.cfg).clone()).to_value());

        let mut base = self.merge_parts(&mut base)?;
        if !iht.is_empty() {
            datapatch::inherit(&mut base, &self.merge_parts(&mut iht)?);
        }

        Ok(serde_yaml::from_value(base)?)
    }
}

/// Load spec from a given path
pub fn load(
    cfg: Arc<MinionConfig>, path: &str, traits: Option<SystemTraits>, context: Option<IndexMap<String, serde_json::Value>>,
) -> Result<ModelSpec, SysinspectError> {
    log::info!("Loading model spec from {path}");
    SpecLoader::new(cfg, fs::canonicalize(path)?, traits, context).load()
}
