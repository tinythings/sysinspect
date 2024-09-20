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
    ModelMultipleIndex(String),
    IoErr(io::Error),
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
        };

        write!(f, "{msg}")?;
        Ok(())
    }
}

impl From<io::Error> for SyspectError {
    fn from(err: io::Error) -> Self {
        SyspectError::IoErr(err)
    }
}
