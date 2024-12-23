use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtoErrorCode {
    /// No code
    Undef = 0,

    /// Successfully completed
    Success = 1,

    /// General unspecified failure
    GeneralFailure = 2,

    /// Minion is not registered
    NotRegistered = 3,

    /// Minion is already registered
    AlreadyRegistered = 4,

    /// Minion is already connected
    AlreadyConnected = 5,

    /// Unassigned, unknown
    Unknown,
}
