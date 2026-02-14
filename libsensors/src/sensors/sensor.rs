use async_trait::async_trait;
use serde_json::Value;
use std::fmt::Debug;

pub type SensorEvent = Value;

#[async_trait]
pub trait Sensor: Debug + Send + Sync {
    fn new(id: String, cfg: crate::sspec::SensorConf) -> Self
    where
        Self: Sized;

    fn id() -> String
    where
        Self: Sized;

    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync));
}
