use super::{datapatch, mspecdef::ModelSpec};
use crate::{cfg::select_config, tmpl::render::ModelTplRender, traits::systraits::SystemTraits, SysinspectError};
use serde_yaml::Value;
use std::{
    collections::HashMap,
    fs::{self},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

pub const MODEL_INDEX: &str = "model.cfg";
pub const MODEL_FILE_EXT: &str = ".cfg";

/// Spec loader object
struct SpecLoader {
    pth: PathBuf,

    // Only one init allowed
    init: bool,

    // System traits if running in distributed mode
    traits: Option<HashMap<String, serde_json::Value>>,
}

impl SpecLoader {
    // Constructor
    fn new(pth: PathBuf, traits: Option<SystemTraits>) -> Self {
        let mut ext: Option<HashMap<String, serde_json::Value>> = None;
        if let Some(traits) = traits {
            let mut et: HashMap<String, serde_json::Value> = HashMap::new();
            for k in traits.items() {
                if let Some(v) = traits.get(&k) {
                    et.insert(k, v);
                }
            }
            ext = Some(et);
        }

        Self { pth, init: false, traits: ext }
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
                        out.insert(0, serde_yaml::from_str::<Value>(&fs::read_to_string(etr.path())?)?);
                    }
                } else if fname != MODEL_INDEX {
                    // Get YAML chunks and render them
                    let mut mtr = ModelTplRender::new(fname, &fs::read_to_string(etr.path())?);
                    if let Some(traits) = self.traits.clone() {
                        mtr.set_ns_values("traits", traits);
                    }

                    match serde_yaml::from_str::<Value>(&mtr.render()?) {
                        Ok(chunk) => out.push(chunk),
                        Err(err) => return Err(SysinspectError::ModelDSLError(format!("Unable to parse {fname}: {err}"))),
                    }
                } else {
                    log::debug!("Skipping inherited index");
                }
            }
        }

        Ok(out)
    }

    /// Merge YAML parts
    fn merge_parts(&mut self, chunks: &mut Vec<Value>) -> Result<Value, SysinspectError> {
        if chunks.is_empty() {
            return Err(SysinspectError::ModelMultipleIndex("No data found".to_string()));
        }

        let mut base = chunks.remove(0);

        for chunk in chunks {
            match (&mut base, chunk) {
                (Value::Mapping(ref mut amp), Value::Mapping(bmp)) => {
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
                        return Err(SysinspectError::ModelDSLError(format!(
                            "Mapping expected, but this structure passed: {:?}\n\t > {:?}",
                            a, b
                        )));
                    }
                }
            }
        }
        Ok(base)
    }

    /// Load model spec by merging all the data parts and validating
    /// its content.
    fn load(&mut self) -> Result<ModelSpec, SysinspectError> {
        let mpt = self.collect_by_path(&self.pth.to_owned(), false)?;
        let mut base: Vec<Value> = Vec::default();
        let mut iht: Vec<Value> = Vec::default();

        // Try inheriting
        if !mpt.is_empty() {
            if let Some(ipth) = serde_yaml::from_value::<ModelSpec>(mpt[0].to_owned())?.inherits() {
                let ipth = fs::canonicalize(self.pth.join(ipth))?; // Redirect path
                base.insert(0, mpt[0].to_owned());
                base.extend(self.collect_by_path(&ipth, true)?);
                iht.extend(mpt[1..].iter().map(|e| e.to_owned()).collect::<Vec<Value>>());
            } else {
                base.extend(mpt);
            }
        } else {
            return Err(SysinspectError::ModelDSLError(format!("No model found at {}", self.pth.to_str().unwrap_or_default())));
        }

        // Load app config and merge to the main model
        base.push(serde_yaml::from_str::<Value>(&fs::read_to_string(select_config(None)?)?)?);

        let mut base = self.merge_parts(&mut base)?;
        if !iht.is_empty() {
            datapatch::inherit(&mut base, &self.merge_parts(&mut iht)?);
        }

        Ok(serde_yaml::from_value(base)?)
    }
}

/// Load spec from a given path
pub fn load(path: &str, traits: Option<SystemTraits>) -> Result<ModelSpec, SysinspectError> {
    SpecLoader::new(fs::canonicalize(path)?, traits).load()
}
