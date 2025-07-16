use libsysinspect::{SysinspectError, cfg::mmconf::MasterConfig, proto::MasterMessage};
use libwebapi::MasterInterface;

use crate::master::SysMaster;

#[async_trait::async_trait]
impl MasterInterface for SysMaster {
    /// Returns a reference to the master configuration.
    async fn cfg(&self) -> &MasterConfig {
        &self.cfg_ref()
    }

    /// Query operation
    async fn query(&mut self, query: String) -> Result<MasterMessage, SysinspectError> {
        if let Some(msg) = self.msg_query(&query) {
            return Ok(msg);
        } else {
            return Err(SysinspectError::InvalidQuery(format!("Invalid query: {}", query)));
        }
    }
}
