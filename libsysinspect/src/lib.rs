use std::{
    error::Error,
    ffi::NulError,
    fmt::{Display, Formatter, Result},
    io,
};

use mdescr::mspec;

pub mod cfg;
pub mod inspector;
pub mod intp;
pub mod logger;
pub mod mdescr;
pub mod modlib;
pub mod proto;
pub mod reactor;
pub mod rsa;
pub mod traits;
pub mod util;

#[derive(Debug)]
pub enum SysinspectError {
    // Specific errors
    ModelMultipleIndex(String),
    ModelDSLError(String),
    ModuleError(String),
    ConfigError(String),
    MasterGeneralError(String),
    MinionGeneralError(String),
    ProtoError(String),

    // Wrappers for the system errors
    IoErr(io::Error),
    SerdeYaml(serde_yaml::Error),
    SerdeJson(serde_json::Error),
    FFINullError(NulError),
    DynError(Box<dyn Error>),
    AsynDynError(Box<dyn Error + Send + Sync>),
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
            SysinspectError::SerdeJson(err) => format!("(JSON) {err}"),
            SysinspectError::ModelDSLError(err) => format!("(DSL) {err}"),
            SysinspectError::ModuleError(err) => format!("(Module) {err}"),
            SysinspectError::ConfigError(err) => format!("(Config) {err}"),
            SysinspectError::FFINullError(err) => format!("(System) {err}"),
            SysinspectError::MasterGeneralError(err) => format!("(Master) {err}"),
            SysinspectError::MinionGeneralError(err) => format!("(Minion) {err}"),
            SysinspectError::ProtoError(err) => format!("(Protocol) {err}"),
            SysinspectError::DynError(err) => format!("(General) {err}"),
            SysinspectError::AsynDynError(err) => format!("(General part) {err}"),
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

/// Handle JSON errors
impl From<serde_json::Error> for SysinspectError {
    fn from(err: serde_json::Error) -> Self {
        SysinspectError::SerdeJson(err)
    }
}

/// Handle FFI Nul error
impl From<NulError> for SysinspectError {
    fn from(err: NulError) -> Self {
        SysinspectError::FFINullError(err)
    }
}

// Implement From<Box<dyn Error>> for SysinspectError
impl From<Box<dyn Error>> for SysinspectError {
    fn from(err: Box<dyn Error>) -> SysinspectError {
        SysinspectError::DynError(err)
    }
}

impl From<Box<dyn Error + Send + Sync>> for SysinspectError {
    fn from(err: Box<dyn Error + Send + Sync>) -> SysinspectError {
        SysinspectError::DynError(err)
    }
}
