use crate::{SysinspectError, intp::actproc::response::ActionResponse, mdescr::telemetry::TelemetrySpec};
use async_trait::async_trait;

#[async_trait]
pub trait EventProcessorCallback: std::fmt::Debug + Send + Sync {
    async fn on_action_response(&mut self, ar: ActionResponse) -> Result<(), SysinspectError>;
    fn set_telemetry_config(&mut self, telemetry_config: Option<TelemetrySpec>);
}
