use crate::{SysinspectError, intp::actproc::response::ActionResponse};
use async_trait::async_trait;

#[async_trait]
pub trait EventProcessorCallback: std::fmt::Debug + Send + Sync {
    async fn on_action_response(&mut self, ar: ActionResponse) -> Result<(), SysinspectError>;
}
