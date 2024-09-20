use std::{
    error::Error,
    fmt::{Display, Formatter, Result},
};

pub mod mdl;
pub mod runtime;
pub mod tpl;

#[derive(Debug)]
pub enum SyspectError {
    ModelMultipleIndex,
}

impl Error for SyspectError {}

impl Display for SyspectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let msg = match self {
            SyspectError::ModelMultipleIndex => "Multiple model.cfg found at this path",
        };

        write!(f, "ERROR: {msg}")?;
        Ok(())
    }
}
