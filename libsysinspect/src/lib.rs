use std::{
    error::Error,
    fmt::{Display, Formatter, Result},
    io,
};

use mdl::mspec;

pub mod mdl;
pub mod runtime;
pub mod tpl;

#[derive(Debug)]
pub enum SyspectError {
    // Specific errors
    ModelMultipleIndex(String),
    ModelDSLError(String),

    // Wrappers for the system errors
    IoErr(io::Error),
    SerdeYaml(serde_yaml::Error),
}

impl Error for SyspectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SyspectError::IoErr(err) => Some(err),
            _ => None,
        }
    }
}

impl Display for SyspectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let msg = match self {
            SyspectError::ModelMultipleIndex(m) => {
                format!("Another {} file found as '{}'", mspec::MODEL_INDEX, m)
            }
            SyspectError::IoErr(err) => format!("(I/O) {err}"),
            SyspectError::SerdeYaml(err) => format!("(YAML) {err}"),
            SyspectError::ModelDSLError(err) => format!("(DSL) {err}"),
        };

        write!(f, "{msg}")?;
        Ok(())
    }
}

/// Handle IO errors
impl From<io::Error> for SyspectError {
    fn from(err: io::Error) -> Self {
        SyspectError::IoErr(err)
    }
}

/// Handle YAML errors
impl From<serde_yaml::Error> for SyspectError {
    fn from(err: serde_yaml::Error) -> Self {
        SyspectError::SerdeYaml(err)
    }
}
