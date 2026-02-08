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
use serde_json::{Value, json};
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[derive(Default, Debug)]
pub struct PipeScriptHandler {
    eid: String,
    cfg: EventConfig,
}

impl PipeScriptHandler {
    /// Format the output
    fn fmt(&self, value: Value, format: &str) -> String {
        match format.to_lowercase().as_str() {
            "yaml" => serde_yaml::to_string(&value).unwrap_or_default(),
            _ => serde_json::to_string(&value).unwrap_or_default(),
        }
    }

    fn wait_with_timeout(child: &mut Child, timeout: Duration) -> std::io::Result<Option<ExitStatus>> {
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(Some(status));
            }
            if start.elapsed() >= timeout {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// Call user-defined script
    fn call_script(&self, evt: &ActionResponse) {
        // Successful responses only
        if evt.response.retcode() != 0 {
            return;
        }

        // Skip events that don't belong
        if !evt.match_eid(&self.eid) {
            log::debug!("Event {} doesn't match handler {}", evt.eid().bright_yellow(), self.eid.bright_yellow());
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

        if cmd.is_empty() {
            return;
        }

        // Verbosity
        let quiet = cfg.as_bool("quiet").unwrap_or(false);

        // Communication format
        let format = cfg.as_string("format").unwrap_or("json".to_string());

        // Timeout (ms) - default 10s
        let timeout = Duration::from_secs(cfg.as_int("timeout").unwrap_or(10).max(0) as u64);

        // Spawn policy:
        // - keep stdin piped for payload
        // - avoid stdout/stderr pipe deadlocks: inherit for noisy mode, null for quiet
        let mut command = Command::new(&cmd[0]);
        command.args(&cmd[1..]).stdin(Stdio::piped());

        if quiet {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        } else {
            command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        }

        match command.spawn() {
            Ok(mut p) => {
                if let Some(mut stdin) = p.stdin.take() {
                    let data = json!({
                        "id.entity": evt.eid(),
                        "id.action": evt.aid(),
                        "id.state": evt.sid(),
                        "ret.code": evt.response.retcode(),
                        "ret.warn": evt.response.warnings(),
                        "ret.info": evt.response.message(),
                        "ret.data": evt.response.data(),
                        "timestamp": evt.ts_rfc_3339(),
                    });

                    if let Err(err) = stdin.write_all(self.fmt(data, &format).as_bytes()) {
                        log::error!("Unable to pipe data to '{}': {}", cmd.join(" "), err);
                    } else if !quiet {
                        log::info!("{} - {}", "Pipescript".cyan(), cmd.join(" "));
                    }
                    // stdin dropped here => EOF for child
                }

                // Always reap; enforce timeout
                match Self::wait_with_timeout(&mut p, timeout) {
                    Ok(Some(status)) => {
                        if !quiet {
                            log::debug!("{} exit: {}", "Pipescript".cyan(), status);
                        }
                    }
                    Ok(None) => {
                        log::error!("{} timeout after {}s: killing '{}'", "Pipescript".cyan(), timeout.as_secs(), cmd.join(" "));
                        let _ = p.kill();
                        let _ = p.wait(); // reap no matter what
                    }
                    Err(e) => {
                        log::error!("{} wait error for '{}': {}", "Pipescript".cyan(), cmd.join(" "), e);
                        let _ = p.wait(); // best-effort reap
                    }
                }
            }
            Err(err) => log::error!("{} error: {} for '{}'", PipeScriptHandler::id(), err, cmd.join(" ")),
        };
    }
}

/// Pipescript handler
impl EventHandler for PipeScriptHandler {
    fn new(eid: String, cfg: crate::intp::conf::EventConfig) -> Self
    where
        Self: Sized,
    {
        PipeScriptHandler { eid, cfg }
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
