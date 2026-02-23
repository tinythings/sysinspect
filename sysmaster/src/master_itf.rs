use std::sync::Arc;

use libcommon::SysinspectError;
use libdatastore::resources::DataStorage;
use libsysinspect::cfg::mmconf::MasterConfig;
use libwebapi::MasterInterface;

use crate::master::SysMaster;

#[async_trait::async_trait]
impl MasterInterface for SysMaster {
    async fn cfg(&self) -> &MasterConfig {
        self.cfg_ref()
    }

    async fn datastore(&self) -> Arc<tokio::sync::Mutex<DataStorage>> {
        self.datastore()
    }

    async fn query(&mut self, query: String) -> Result<(), SysinspectError> {
        let Some(msg) = self.msg_query(&query).await else {
            return Err(SysinspectError::InvalidQuery(format!("Invalid query: {query}")));
        };

        let Some(master) = self.as_ptr() else {
            return Err(SysinspectError::InvalidQuery("Master pointer is not set".to_string()));
        };

        SysMaster::bcast_master_msg(&self.broadcast(), self.cfg_ref().telemetry_enabled(), master.clone(), Some(msg.clone())).await;

        {
            let master_guard = master.lock().await;
            let ids = master_guard.get_minion_registry().lock().await.get_targeted_minions(msg.target(), false).await;
            log::error!("Targeted minions: {:#?}", ids);
        }

        Ok(())
    }
}
