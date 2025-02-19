use crate::{intp::actproc::response::ActionResponse, SysinspectError};
use async_trait::async_trait;

#[async_trait]
pub trait AsyncEventProcessorCallback: std::fmt::Debug + Send + Sync {
    async fn on_action_response(&mut self, ar: ActionResponse) -> Result<(), SysinspectError>;
}
