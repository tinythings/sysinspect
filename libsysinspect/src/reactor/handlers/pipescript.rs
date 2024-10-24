/*
Pipescript is a handler that pipes the action output through a script
on certain event outcomes.
 */

use super::evthandler::EventHandler;
use crate::intp::{
    actproc::response::ActionResponse,
    conf::{EventConfig, EventConfigOption},
};
use colored::Colorize;
use core::str;
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[derive(Default, Debug)]
pub struct PipeScriptHandler {
    cfg: EventConfig,
}

impl PipeScriptHandler {
    /// Format the output
    fn fmt(&self, value: Option<Value>, format: &str) -> String {
        let value = match value {
            Some(v) => v,
            None => return "".to_string(),
        };

        match format.to_lowercase().as_str() {
            "yaml" => serde_yaml::to_string(&value).unwrap_or_default(),
            _ => serde_json::to_string(&value).unwrap_or_default(),
        }
    }

    /// Call user-defined script
    fn call_script(&self, evt: &ActionResponse) {
        // Successfull responses only
        if evt.response.retcode() > 0 {
            return;
        }

        // Config is required
        let cfg = match self.config() {
            Some(cfg) => cfg,
            None => return,
        };

        // Program is required
        let cmd = match cfg.as_string("program") {
            Some(cmd) => cmd.split_whitespace().map(|s| s.to_string()).collect::<Vec<String>>(),
            None => return,
        };

        // Verbosity
        let quiet = cfg.as_bool("quiet").unwrap_or(false);

        // Communication format
        let format = cfg.as_string("format").unwrap_or("json".to_string());

        match Command::new(&cmd[0]).args(&cmd[1..]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
            Ok(mut p) => {
                if let Some(mut stdin) = p.stdin.take() {
                    if let Err(err) = stdin.write_all(self.fmt(evt.response.data(), &format).as_bytes()) {
                        log::error!("Unable to pipe data to '{}': {}", cmd.join(" "), err);
                    } else if !quiet {
                        log::info!("{} - {}", "Pipescript".cyan(), cmd.join(" "));
                    }
                }
            }
            Err(err) => log::error!("{} error: {} for '{}'", PipeScriptHandler::id(), err, cmd.join(" ")),
        };
    }
}

/// Pipescript handler
impl EventHandler for PipeScriptHandler {
    fn new(_eid: String, cfg: crate::intp::conf::EventConfig) -> Self
    where
        Self: Sized,
    {
        PipeScriptHandler { cfg }
    }

    fn id() -> String
    where
        Self: Sized,
    {
        "pipescript".to_string()
    }

    fn handle(&self, evt: &ActionResponse) {
        self.call_script(evt);
    }

    fn config(&self) -> Option<EventConfigOption> {
        self.cfg.cfg(&PipeScriptHandler::id())
    }
}
