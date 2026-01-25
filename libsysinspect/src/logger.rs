use std::{io::IsTerminal, sync::Mutex};

use chrono::Local;
use colored::{self, Colorize};
use console::strip_ansi_codes;
use log::{Level, Metadata, Record};

#[derive(Default)]
pub struct STDOUTLogger {
    nocolor: bool,
}

impl STDOUTLogger {
    pub fn new(nocolor: bool) -> STDOUTLogger {
        STDOUTLogger { nocolor }
    }
}

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

            let mut msg = format!("[{}] - {}: {}", Local::now().format("%d/%m/%Y %H:%M:%S"), s_level, msg.args());
            if self.nocolor || !std::io::stdout().is_terminal() {
                msg = strip_ansi_codes(msg.as_str()).into_owned();
            }
            println!("{}", msg);
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

/// Forward a log line to the internal logger
/// This is used to forward logs from subprocesses (modules, typically) or external tools.
/// A module would return a structure with "data" and "logs" fields, where "logs" is a list of log lines
/// in the expected format. This function would be called for each log line to forward it to the main logger.
///
/// IMPORTANT: The formatting must be equal!
///
/// Expected wire format:
/// ```
/// "[..timestamp..] - LEVEL: [highlight] message"
/// ```
///
/// Example log line:
/// ```
/// 2024-10-05 14:23:01 - INFO: [ModuleXYZ] This is a log message
/// ```
pub fn log_forward(line: &str) {
    // timestamp
    let after_ts = match line.split_once(" - ") {
        Some((_ts, rest)) => rest,
        None => line,
    };

    // level
    let (level, msg) = match after_ts.split_once(':') {
        Some((lvl, rest)) => (lvl.trim(), rest.trim()),
        None => ("INFO", after_ts.trim()),
    };

    // Highlight leading [xxx]
    let painted_msg = if let Some(rest) = msg.strip_prefix('[') {
        if let Some((tag, tail)) = rest.split_once(']') {
            let tag = format!("[{}]", tag).bright_magenta();
            let tail = tail.trim_start(); // eat leading space
            format!("{tag} {tail}")
        } else {
            msg.to_string()
        }
    } else {
        msg.to_string()
    };

    match level {
        "ERROR" => log::error!("{painted_msg}"),
        "WARN" | "WARNING" => log::warn!("{painted_msg}"),
        "DEBUG" => log::debug!("{painted_msg}"),
        "TRACE" => log::trace!("{painted_msg}"),
        "INFO" => log::info!("{painted_msg}"),
        _ => log::info!("{painted_msg}"),
    }
}
