use libcommon::SysinspectError;
use libsysinspect::cfg::mmconf::MasterConfig;
use libwebapi::MasterInterface;

use crate::master::SysMaster;

#[async_trait::async_trait]
impl MasterInterface for SysMaster {
    /// Returns a reference to the master configuration.
    async fn cfg(&self) -> &MasterConfig {
        &self.cfg_ref()
    }

    /// Query operation
    async fn query(&mut self, query: String) -> Result<(), SysinspectError> {
        if let Some(msg) = self.msg_query(&query).await {
            if let Some(master) = self.as_ptr() {
                // `bcast_master_msg` takes ownership of the master handle; keep a copy for later use.
                SysMaster::bcast_master_msg(&self.broadcast(), self.cfg_ref().telemetry_enabled(), master.clone(), Some(msg.clone())).await;
                {
                    let master_guard = master.lock().await;
                    let ids = master_guard.get_minion_registry().lock().await.get_targeted_minions(msg.target(), false).await;
                    log::error!("Targeted minions: {:#?}", ids);
                }
            } else {
                return Err(SysinspectError::InvalidQuery("Master pointer is not set".to_string()));
            }
        } else {
            return Err(SysinspectError::InvalidQuery(format!("Invalid query: {query}")));
        }
        Ok(())
    }
}
