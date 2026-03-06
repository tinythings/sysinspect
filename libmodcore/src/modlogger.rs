use chrono::Local;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::sync::{Mutex, OnceLock};

static LOGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static MODULE_ID: OnceLock<String> = OnceLock::new();

pub struct VecLogger;

impl Log for VecLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        if let Some(store) = LOGS.get() {
            let mut vec = store.lock().unwrap();

            let mid = MODULE_ID.get().map(|s| s.as_str()).unwrap_or("unknown");

            vec.push(format!("[{}] [{}] - {}: {}", Local::now().format("%d/%m/%Y %H:%M:%S"), mid, record.level(), record.args()));
        }
    }

    fn flush(&self) {}
}

static LOGGER: VecLogger = VecLogger;

/// Call once at startup
pub fn init_module_logger(mid: &str) {
    LOGS.get_or_init(|| Mutex::new(Vec::new()));
    MODULE_ID.get_or_init(|| mid.to_string());

    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Trace);
}

/// Drain logs
pub fn take_logs() -> Vec<String> {
    if let Some(store) = LOGS.get() {
        let mut vec = store.lock().unwrap();
        std::mem::take(&mut *vec)
    } else {
        vec![]
    }
}
