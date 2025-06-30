/*
Events scheduler is a cronjob-like service.
 */

use libsysinspect::{
    SysinspectError,
    cfg::mmconf::{
        CFG_TASK_INTERVAL_DAYS, CFG_TASK_INTERVAL_HOURS, CFG_TASK_INTERVAL_MINUTES, CFG_TASK_INTERVAL_SECONDS, TaskConfig,
    },
};
use tokio::sync::broadcast::{Receiver, channel};
use tokio_task_scheduler::{Scheduler, Task, TaskBuilder};

pub struct EventTask {
    task: Task,
}

impl EventTask {
    pub fn new<F, Fut>(cfg: TaskConfig, callback: F) -> Result<Self, SysinspectError>
    where
        F: Fn() -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let b = TaskBuilder::new(cfg.name(), {
            let callback = callback.clone();
            move || {
                let callback = callback.clone();
                tokio::spawn(async move {
                    callback().await;
                });
                Ok(())
            }
        });

        let (i, u) = cfg.interval();
        let b = match u.as_str() {
            CFG_TASK_INTERVAL_SECONDS => b.every_seconds(i),
            CFG_TASK_INTERVAL_MINUTES => b.every_minutes(i),
            CFG_TASK_INTERVAL_HOURS => b.every_minutes(i * 60),
            CFG_TASK_INTERVAL_DAYS => b.every_minutes(i * 60 * 24),
            _ => {
                return Err(SysinspectError::MasterGeneralError(format!("Invalid interval unit: {u}")));
            }
        };

        Ok(Self { task: b.build() })
    }

    pub fn id(&self) -> &str {
        self.task.id()
    }
}

pub(crate) struct EventsScheduler {
    scheduler: Scheduler,
    bcast: Receiver<()>,
}

impl EventsScheduler {
    pub fn new() -> Self {
        Self { scheduler: Scheduler::new(), bcast: channel(100).1 }
    }

    pub async fn add(&mut self, event: EventTask) -> Result<(), SysinspectError> {
        self.scheduler
            .add_task(event.task.clone())
            .await
            .map(|_| ())
            .map_err(|e| SysinspectError::MasterGeneralError(e.to_string()))
    }

    pub async fn remove(&mut self, id: &str) -> Result<(), SysinspectError> {
        self.scheduler.remove(id).await.map(|_| ()).map_err(|e| SysinspectError::MasterGeneralError(e.to_string()))
    }

    pub async fn start(&mut self) -> Result<(), SysinspectError> {
        self.bcast = self.scheduler.start().await;
        Ok(())
    }
}
