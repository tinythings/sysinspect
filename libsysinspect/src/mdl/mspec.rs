use crate::SyspectError;
use serde_yaml::{Mapping, Value};
use std::{collections::BTreeMap, fs, path::PathBuf};
use walkdir::WalkDir;

/// Model Specification
/// ===================
///
/// This module's job is to read all the model files and put them together
/// into one tree, resolving all interpolative points to one single
/// configuration (spec)

pub struct ModelSpec {
    // These are fields of model.cfg init config
    //
    // Model name
    name: String,

    // Model version
    version: String,

    // A multi-line description of the model, used for reports
    // or other places.
    description: String,

    // Model maintainer
    maintainer: String,
}

pub const MODEL_INDEX: &str = "model.cfg";

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
    fn collect_parts(&mut self) -> Result<Vec<Value>, SyspectError> {
        let mut out = Vec::<Value>::new();

        for etr in WalkDir::new(&self.pth).follow_links(true).into_iter().filter_map(Result::ok) {
            // Skip dirs
            if !etr.path().is_file() {
                continue;
            }

            // Crash if multiple indexes found (or we are at multuple models)
            if let Some(fname) = etr.path().file_name().and_then(|s| s.to_str()) {
                if fname == MODEL_INDEX {
                    if self.init {
                        return Err(SyspectError::ModelMultipleIndex(etr.path().as_os_str().to_str().unwrap().to_string()));
                    } else {
                        self.init = true;
                        out.insert(0, serde_yaml::from_str::<Value>(&fs::read_to_string(etr.path())?)?);
                    }
                } else {
                    // Get YAML chunks
                    out.push(serde_yaml::from_str::<Value>(&fs::read_to_string(etr.path())?)?);
                }
            }
        }

        Ok(out)
    }

    /// Merge YAML parts
    fn merge_parts(&mut self, chunks: &mut Vec<Value>) -> Result<Value, SyspectError> {
        if chunks.len() == 0 {
            return Err(SyspectError::ModelMultipleIndex("blah happened".to_string()));
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
                    println!("UNSUPPORTED>>> {:?}\n\t-> {:?}\n", a, b); // XXX: Probably exception too
                }
            }
        }
        Ok(base)
    }

    /// Load model spec by merging all the data parts and validating
    /// its content.
    fn load(&mut self) -> Result<ModelSpec, SyspectError> {
        let mut parts = self.collect_parts()?;
        let base = self.merge_parts(&mut parts)?;
        println!("........\n{}\n.......", serde_yaml::to_string(&base)?);

        Ok(ModelSpec {
            name: "test".to_string(),
            version: "0.1".to_string(),
            description: "Blah".to_string(),
            maintainer: "me and you too".to_string(),
        })
    }
}

/// Load spec from a given path
pub fn load(path: &str) -> Result<ModelSpec, SyspectError> {
    Ok(SpecLoader::new(path).load()?)
}
