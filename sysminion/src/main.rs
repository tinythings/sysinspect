mod arcb;
mod clidef;
mod filedata;
mod minion;
mod proto;
mod rsa;

use clap::{ArgMatches, Command};
use clidef::cli;
use daemonize::Daemonize;
use libsysinspect::{
    SysinspectError,
    cfg::{get_minion_config, mmconf::MinionConfig},
    logger,
};
use log::LevelFilter;
use std::{env, fs::File, process::exit};
use tokio::task::JoinHandle;

static APPNAME: &str = "sysminion";
static VERSION: &str = "0.4.0";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;

fn start_minion(cfg: MinionConfig, fp: Option<String>) -> Result<(), SysinspectError> {
    let runtime = tokio::runtime::Runtime::new().map_err(|e| SysinspectError::DynError(Box::new(e)))?;
    runtime.block_on(async {
        loop {
            let c_cfg = cfg.clone();
            let c_fp = fp.clone();
            let h: JoinHandle<()> = tokio::spawn(async move {
                minion::minion(c_cfg, c_fp).await;
            });

            log::info!("Minion process started");

            match h.await {
                Ok(_) => println!("Minion task completed."),
                Err(e) if e.is_cancelled() => println!("Minion task was aborted."),
                Err(e) => println!("Minion task failed: {:?}", e),
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    Ok(())
}

fn get_config(params: &ArgMatches) -> MinionConfig {
    // Config
    match get_minion_config(Some(params.get_one::<String>("config").map_or("", |v| v))) {
        Ok(cfg) => cfg,
        Err(err) => {
            log::error!("Unable to find a Minion config: {err}");
            exit(1);
        }
    }
}

// Print help?
fn help(cli: &mut Command, params: ArgMatches) -> bool {
    if let Some(sub) = params.subcommand_matches("setup") {
        if sub.get_flag("help") {
            if let Some(s_cli) = cli.find_subcommand_mut("setup") {
                _ = s_cli.print_help();
                return true;
            }
            return false;
        }
    }
    if params.get_flag("help") {
        _ = &cli.print_long_help();
        return true;
    }

    // Print a global version?
    if params.get_flag("version") {
        println!("Version: {} {}", APPNAME, VERSION);
        return true;
    }

    false
}

fn main() -> std::io::Result<()> {
    let mut cli = cli(VERSION, APPNAME);
    if env::args().collect::<Vec<String>>().len() == 1 {
        cli.print_help()?;
        std::process::exit(1);
    }

    let params = cli.to_owned().get_matches();

    // Print helps, versions etc
    if help(&mut cli, params.clone()) {
        std::process::exit(0);
    }

    // Setup logger
    if let Err(err) = log::set_logger(&LOGGER).map(|()| {
        log::set_max_level(match params.get_count("debug") {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            2.. => LevelFilter::max(),
        })
    }) {
        println!("Error setting logger output: {}", err);
    }

    // Start
    let fp = params.get_one::<String>("register").cloned();
    if params.get_flag("start") || fp.is_some() {
        let cfg = get_config(&params);
        if let Err(err) = start_minion(cfg, fp) {
            log::error!("Error starting minion: {err}");
        }
    } else if params.get_flag("daemon") {
        log::info!("Starting daemon");
        let cfg = get_config(&params);
        let sout = match File::create(cfg.logfile_std()) {
            Ok(sout) => {
                log::info!("Opened main log file at {}", cfg.logfile_std().to_str().unwrap_or_default());
                sout
            }
            Err(err) => {
                log::error!(
                    "Unable to create main log file at {}: {err}, terminating",
                    cfg.logfile_std().to_str().unwrap_or_default()
                );
                exit(1);
            }
        };
        let serr = match File::create(cfg.logfile_err()) {
            Ok(serr) => {
                log::info!("Opened error log file at {}", cfg.logfile_err().to_str().unwrap_or_default());

                serr
            }
            Err(err) => {
                log::error!("Unable to create file at {}: {err}, terminating", cfg.logfile_err().to_str().unwrap_or_default());
                exit(1);
            }
        };

        match Daemonize::new().pid_file(cfg.pidfile()).stdout(sout).stderr(serr).start() {
            Ok(_) => {
                log::info!("Daemon started with PID file at {}", cfg.pidfile().to_str().unwrap_or_default());
                if let Err(err) = start_minion(cfg, fp) {
                    log::error!("Error starting minion: {err}");
                }
            }
            Err(err) => {
                log::error!("Error starting minion in daemon mode: {err}");
                exit(1)
            }
        }
    } else if params.get_flag("stop") {
        log::info!("Stopping daemon");
        let cfg = get_config(&params);
        if let Err(err) = libsysinspect::util::sys::kill_process(cfg.pidfile(), Some(2)) {
            log::error!("Unable to stop sysminion: {err}");
        }
    } else if let Some(sub) = params.subcommand_matches("setup") {
        if let Err(err) = minion::setup(sub) {
            log::error!("Error running setup: {err}");
        }
    } else {
        cli.print_help()?;
    }

    Ok(())
}
