use crate::{
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use libmenotify::{MeNotifyContext, MeNotifyEntrypoint, MeNotifyEventBuilder, MeNotifyRunner, MeNotifyRuntime};
use std::{fmt, time::Duration};

pub struct MeNotifySensor {
    cfg: SensorConf,
    runtime: MeNotifyRuntime,
}

impl fmt::Debug for MeNotifySensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeNotifySensor").field("sid", &self.runtime.sid()).field("listener", &self.runtime.listener()).finish()
    }
}

#[async_trait]
impl Sensor for MeNotifySensor {
    fn new(id: String, cfg: SensorConf) -> Self {
        Self { runtime: MeNotifyRuntime::new(id, cfg.listener().to_string()), cfg }
    }

    fn id() -> String {
        "menotify".to_string()
    }

    async fn run(&self, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        match self.runtime.load_program() {
            Ok(program) => {
                let runner = self.runner(program);
                match runner.entrypoint() {
                    MeNotifyEntrypoint::Tick => self.run_tick(runner, emit),
                    MeNotifyEntrypoint::Loop => self.run_loop(runner, emit),
                }
            }
            Err(err) => self.runtime.log_bootstrap_error(&err),
        }
    }
}

impl MeNotifySensor {
    /// Returns the polling interval used for `tick(ctx)` execution.
    ///
    /// # Returns
    ///
    /// Returns the configured interval, or a conservative fallback.
    fn interval(&self) -> Duration {
        self.cfg.interval().unwrap_or_else(|| Duration::from_secs(3))
    }

    /// Builds a runner for the configured sensor instance.
    ///
    /// # Arguments
    ///
    /// * `program` - Loaded MeNotify program.
    ///
    /// # Returns
    ///
    /// Returns a new `MeNotifyRunner`.
    fn runner(&self, program: libmenotify::MeNotifyProgram) -> MeNotifyRunner {
        MeNotifyRunner::new(
            program,
            MeNotifyContext::new(
                self.runtime.sid(),
                self.runtime.listener(),
                self.runtime.module_name().unwrap_or_default(),
                self.cfg.opts(),
                self.cfg.args(),
                self.cfg.interval(),
            ),
        )
    }

    /// Builds an event envelope builder for this sensor instance.
    ///
    /// # Returns
    ///
    /// Returns a `MeNotifyEventBuilder`.
    fn event_builder(&self) -> MeNotifyEventBuilder {
        MeNotifyEventBuilder::new(self.runtime.sid(), self.runtime.listener(), self.cfg.tag())
    }

    /// Runs one `loop(ctx)` style sensor.
    ///
    /// # Arguments
    ///
    /// * `runner` - Prepared runner for the configured sensor.
    ///
    /// # Returns
    ///
    /// Returns nothing. The sensor logs and stops if Lua returns an error.
    fn run_loop(&self, runner: MeNotifyRunner, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        log::info!("[{}] '{}' running module '{}' as loop(ctx)", Self::id().bright_magenta(), self.runtime.sid(), runner.program().module_name());

        if let Err(err) = runner.run_loop_with_emit(emit, &self.event_builder()) {
            log::warn!(
                "[{}] '{}' loop(ctx) failed for module '{}': {}",
                Self::id().bright_magenta(),
                self.runtime.sid(),
                runner.program().module_name(),
                err
            );
        }
    }

    /// Runs one `tick(ctx)` style sensor.
    ///
    /// # Arguments
    ///
    /// * `runner` - Prepared runner for the configured sensor.
    ///
    /// # Returns
    ///
    /// Returns nothing. The sensor keeps ticking until the Lua entrypoint
    /// fails, then logs and stops.
    fn run_tick(&self, runner: MeNotifyRunner, emit: &(dyn Fn(SensorEvent) + Send + Sync)) {
        log::info!(
            "[{}] '{}' running module '{}' as tick(ctx) every {:?}",
            Self::id().bright_magenta(),
            self.runtime.sid(),
            runner.program().module_name(),
            self.interval()
        );

        loop {
            if let Err(err) = runner.run_tick_with_emit(emit, &self.event_builder()) {
                log::warn!(
                    "[{}] '{}' tick(ctx) failed for module '{}': {}",
                    Self::id().bright_magenta(),
                    self.runtime.sid(),
                    runner.program().module_name(),
                    err
                );
                return;
            }
            std::thread::sleep(self.interval());
        }
    }
}
