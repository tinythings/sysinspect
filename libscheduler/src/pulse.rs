/*
Events scheduler is a cronjob-like service.
 */

use libsysinspect::SysinspectError;
use tokio::sync::broadcast::{Receiver, channel};
use tokio_task_scheduler::{Scheduler, Task, TaskBuilder};

pub struct EventTask {
    task: Task,
}

impl EventTask {
    pub fn new<F, Fut>(name: &str, callback: F) -> Self
    where
        F: Fn() -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let name = name.to_string();
        let task = TaskBuilder::new(&name.clone(), {
            let callback = callback.clone();
            move || {
                //let name = name.clone();
                let callback = callback.clone();
                tokio::spawn(async move {
                    callback().await;
                });
                Ok(())
            }
        })
        .every_seconds(5)
        .build();
        Self { task }
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
