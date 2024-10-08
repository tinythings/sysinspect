use std::{
    error::Error,
    fmt::{Display, Formatter, Result},
    io,
};

use mdescr::mspec;

pub mod intp;
pub mod logger;
pub mod mdescr;
pub mod modlib;
pub mod reactor;

#[derive(Debug)]
pub enum SysinspectError {
    // Specific errors
    ModelMultipleIndex(String),
    ModelDSLError(String),
    ModuleError(String),

    // Wrappers for the system errors
    IoErr(io::Error),
    SerdeYaml(serde_yaml::Error),
}

impl Error for SysinspectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SysinspectError::IoErr(err) => Some(err),
            _ => None,
        }
    }
}

impl Display for SysinspectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let msg = match self {
            SysinspectError::ModelMultipleIndex(m) => {
                format!("Another {} file found as '{}'", mspec::MODEL_INDEX, m)
            }
            SysinspectError::IoErr(err) => format!("(I/O) {err}"),
            SysinspectError::SerdeYaml(err) => format!("(YAML) {err}"),
            SysinspectError::ModelDSLError(err) => format!("(DSL) {err}"),
            SysinspectError::ModuleError(err) => format!("(Module) {err}"),
        };

        write!(f, "{msg}")?;
        Ok(())
    }
}

/// Handle IO errors
impl From<io::Error> for SysinspectError {
    fn from(err: io::Error) -> Self {
        SysinspectError::IoErr(err)
    }
}

/// Handle YAML errors
impl From<serde_yaml::Error> for SysinspectError {
    fn from(err: serde_yaml::Error) -> Self {
        SysinspectError::SerdeYaml(err)
    }
}
