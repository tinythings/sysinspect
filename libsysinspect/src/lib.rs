use std::{
    error::Error,
    ffi::NulError,
    io,
};

use jsonpath_rust::parser::errors::JsonPathError;
use mdescr::mspec;
use thiserror::Error;

pub mod cfg;
pub mod inspector;
pub mod intp;
pub mod logger;
pub mod mdescr;
pub mod proto;
pub mod pylang;
pub mod reactor;
pub mod rsa;
pub mod tmpl;
pub mod traits;
pub mod util;

#[derive(Debug, Error)]
pub enum SysinspectError {
    // Specific errors
    #[error("Another {} file found as '{}'", mspec::MODEL_INDEX, .0)]
    ModelMultipleIndex(String),
    #[error("Error loading model DSL: {0}")]
    ModelDSLError(String),
    #[error("Error loading module: {0}")]
    ModuleError(String),
    #[error("Error loading config: {0}")]
    ConfigError(String),
    #[error("Error loading master data: {0}")]
    MasterGeneralError(String),
    #[error("Error loading minion data: {0}")]
    MinionGeneralError(String),
    #[error("Error loading protocol data: {0}")]
    ProtoError(String),
    #[error("Invalid module name: {0}")]
    InvalidModuleName(String),

    // Wrappers for the system errors
    #[error(transparent)]
    IoErr(#[from] io::Error),
    #[error(transparent)]
    SerdeYaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    FFINullError(#[from] NulError),
    #[error(transparent)]
    DynError(#[from] Box<dyn Error + Send + Sync>),
    #[error(transparent)]
    TemplateError(#[from] tera::Error),
    #[error(transparent)]
    SledError(#[from] sled::Error),
    #[error(transparent)]
    AnyError(#[from] anyhow::Error),
    #[error(transparent)]
    JsonPathError(#[from] JsonPathError),
    #[error("Invalid JSONPath: {0}")]
    JsonPathInfo(String),
}
