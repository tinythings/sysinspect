use super::mspecdef::ModelSpec;
use crate::SysinspectError;
use serde_yaml::Value;
use std::{fs, path::PathBuf};
use walkdir::WalkDir;

pub const MODEL_INDEX: &str = "model.cfg";
pub const MODEL_FILE_EXT: &str = ".cfg";

/// Spec loader object
struct SpecLoader {
    pth: PathBuf,

    // Only one init allowed
    init: bool,
}

impl SpecLoader {
    // Constructor
    fn new(pth: &str) -> Self {
        Self { pth: PathBuf::from(pth), init: false }
    }

    /// Collect YAML parts of the model from a different files
    fn collect_parts(&mut self) -> Result<Vec<Value>, SysinspectError> {
        let mut out = Vec::<Value>::new();

        for etr in WalkDir::new(&self.pth).follow_links(true).into_iter().filter_map(Result::ok) {
            // Skip dirs
            if !etr.path().is_file() {
                continue;
            }

            // Crash if multiple indexes found (or we are at multuple models)
            if let Some(fname) = etr.path().file_name().and_then(|s| s.to_str()) {
                if !fname.ends_with(MODEL_FILE_EXT) {
                    continue;
                }

                if fname == MODEL_INDEX {
                    if self.init {
                        return Err(SysinspectError::ModelMultipleIndex(etr.path().as_os_str().to_str().unwrap().to_string()));
                    } else {
                        self.init = true;
                        out.insert(0, serde_yaml::from_str::<Value>(&fs::read_to_string(etr.path())?)?);
                    }
                } else {
                    // Get YAML chunks
                    match serde_yaml::from_str::<Value>(&fs::read_to_string(etr.path())?) {
                        Ok(chunk) => out.push(chunk),
                        Err(err) => return Err(SysinspectError::ModelDSLError(format!("Unable to parse {fname}: {err}"))),
                    }
                }
            }
        }

        Ok(out)
    }

    /// Merge YAML parts
    fn merge_parts(&mut self, chunks: &mut Vec<Value>) -> Result<Value, SysinspectError> {
        if chunks.is_empty() {
            return Err(SysinspectError::ModelMultipleIndex("Multiple index error".to_string()));
            // XXX: Add one more exception
        }

        let mut base = chunks.remove(0);

        for chunk in chunks {
            match (&mut base, chunk) {
                (Value::Mapping(ref mut amp), Value::Mapping(bmp)) => {
                    for (k, v) in bmp {
                        if let Some(av) = amp.get_mut(k) {
                            if let (Some(asq), Some(bsq)) = (av.as_sequence_mut(), v.as_sequence()) {
                                asq.extend(bsq.iter().cloned());
                            } else {
                                *av = v.clone();
                            }
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
        let mut parts = self.collect_parts()?;
        Ok(serde_yaml::from_value(self.merge_parts(&mut parts)?)?)
    }
}

/// Load spec from a given path
pub fn load(path: &str) -> Result<ModelSpec, SysinspectError> {
    SpecLoader::new(path).load()
}
