use crate::{
    sensors::sensor::{Sensor, SensorEvent},
    sspec::SensorConf,
};
use async_trait::async_trait;
use colored::Colorize;
use libmenotify::{MeNotifyEntrypoint, MeNotifyEventBuilder, MeNotifyRunner, MeNotifyRuntime};
use std::{
    collections::HashMap,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{LazyLock, Mutex},
    time::Duration,
};
use tokio::task::block_in_place;

static GENERATIONS: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

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
        let generation = self.activate_generation();
        match self.runtime.load_program() {
            Ok(program) => {
                if !self.generation_is_current(generation) {
                    return;
                }
                let runner = self.runner(program);
                match runner.entrypoint() {
                    MeNotifyEntrypoint::Tick => self.run_tick(runner, emit, generation),
                    MeNotifyEntrypoint::Loop => self.run_loop(runner, emit, generation),
                }
            }
            Err(err) => self.runtime.log_bootstrap_error(&err),
        }
    }
}

impl MeNotifySensor {
    fn activate_generation(&self) -> u64 {
        let mut generations = GENERATIONS.lock().unwrap_or_else(|e| e.into_inner());
        let generation = generations.get(self.runtime.sid()).copied().unwrap_or_default() + 1;
        generations.insert(self.runtime.sid().to_string(), generation);
        generation
    }

    fn generation_is_current(&self, generation: u64) -> bool {
        GENERATIONS.lock().unwrap_or_else(|e| e.into_inner()).get(self.runtime.sid()).copied().unwrap_or_default() == generation
    }

    pub fn invalidate_all() {
        let mut generations = GENERATIONS.lock().unwrap_or_else(|e| e.into_inner());
        for generation in generations.values_mut() {
            *generation += 1;
        }
    }

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
        MeNotifyRunner::with_fresh_state(
            program,
            self.runtime.sid(),
            self.runtime.listener(),
            self.runtime.module_name().unwrap_or_default(),
            self.cfg.opts(),
            self.cfg.args(),
            self.cfg.interval(),
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
    fn run_loop_once(&self, runner: &MeNotifyRunner, emit: &(dyn Fn(SensorEvent) + Send + Sync), builder: &MeNotifyEventBuilder) {
        match block_in_place(|| catch_unwind(AssertUnwindSafe(|| runner.run_loop_with_emit(emit, builder)))) {
            Ok(Ok(())) => (),
            Ok(Err(err)) => log::error!(
                "[{}] '{}' loop(ctx) failed for module '{}': {}",
                Self::id().bright_magenta(),
                self.runtime.sid(),
                runner.program().module_name(),
                err
            ),
            Err(_) => log::error!(
                "[{}] '{}' loop(ctx) panicked for module '{}'",
                Self::id().bright_magenta(),
                self.runtime.sid(),
                runner.program().module_name()
            ),
        }
    }

    fn run_loop(&self, runner: MeNotifyRunner, emit: &(dyn Fn(SensorEvent) + Send + Sync), generation: u64) {
        if !self.generation_is_current(generation) {
            return;
        }
        let builder = self.event_builder();
        log::info!(
            "[{}] '{}' started '{}' with push event listener",
            Self::id().bright_magenta(),
            self.runtime.sid(),
            runner.program().module_name()
        );
        self.run_loop_once(&runner, emit, &builder);
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
    fn run_tick_once(&self, runner: &MeNotifyRunner, emit: &(dyn Fn(SensorEvent) + Send + Sync), builder: &MeNotifyEventBuilder) -> bool {
        match block_in_place(|| catch_unwind(AssertUnwindSafe(|| runner.run_tick_with_emit(emit, builder)))) {
            Ok(Ok(())) => true,
            Ok(Err(err)) => {
                log::error!(
                    "[{}] '{}' tick(ctx) failed for module '{}': {}",
                    Self::id().bright_magenta(),
                    self.runtime.sid(),
                    runner.program().module_name(),
                    err
                );
                false
            }
            Err(_) => {
                log::error!(
                    "[{}] '{}' tick(ctx) panicked for module '{}'",
                    Self::id().bright_magenta(),
                    self.runtime.sid(),
                    runner.program().module_name()
                );
                false
            }
        }
    }

    fn sleep_interval(&self, interval: Duration, generation: u64) -> bool {
        let step = Duration::from_millis(100);
        let mut left = interval;
        while left > Duration::ZERO {
            if !self.generation_is_current(generation) {
                return false;
            }
            let nap = left.min(step);
            block_in_place(|| std::thread::sleep(nap));
            left = left.saturating_sub(nap);
        }
        self.generation_is_current(generation)
    }

    fn run_tick(&self, runner: MeNotifyRunner, emit: &(dyn Fn(SensorEvent) + Send + Sync), generation: u64) {
        let builder = self.event_builder();
        let interval = self.interval();
        log::info!(
            "[{}] '{}' started '{}' with polling {:?}",
            Self::id().bright_magenta(),
            self.runtime.sid(),
            runner.program().module_name(),
            interval
        );

        loop {
            if !self.generation_is_current(generation) {
                return;
            }
            if !self.run_tick_once(&runner, emit, &builder) {
                return;
            }
            if !self.sleep_interval(interval, generation) {
                return;
            }
        }
    }
}
