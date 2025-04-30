use std::sync::Mutex;

use chrono::Local;
use colored::{self, Colorize};
use log::{Level, Metadata, Record};

pub struct STDOUTLogger;

impl log::Log for STDOUTLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, msg: &Record) {
        if self.enabled(msg.metadata()) {
            let s_level: String = match msg.level() {
                log::Level::Info => format!("{}", msg.level().as_str().bright_green()),
                log::Level::Warn => format!("{}", msg.level().as_str().yellow()),
                log::Level::Error => format!("{}", msg.level().as_str().bright_red()),
                log::Level::Debug => format!("{}", msg.level().as_str().bright_cyan()),
                log::Level::Trace => format!("{}", msg.level().as_str().cyan()),
            };

            println!("[{}] - {}: {}", Local::now().format("%d/%m/%Y %H:%M:%S"), s_level, msg.args());
        }
    }

    fn flush(&self) {}
}

#[derive(Default)]
pub struct MemoryLogger {
    pub messages: Mutex<Vec<String>>,
}

impl MemoryLogger {
    // Retrieve the stored messages
    pub fn get_messages(&self) -> Vec<String> {
        self.messages.lock().unwrap().clone()
    }
}

// Implement the Log trait for MemoryLogger
impl log::Log for MemoryLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let mut messages = self.messages.lock().unwrap();
            messages.push(format!("{} - {}", record.level(), record.args()));
        }
    }

    fn flush(&self) {}
}
