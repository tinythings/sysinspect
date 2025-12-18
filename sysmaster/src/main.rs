mod clidef;
mod cluster;
mod dataserv;
mod master;
mod master_itf;
mod registry;
mod telemetry;

use clap::{ArgMatches, Command};
use clidef::cli;
use daemonize::Daemonize;
use libsysinspect::{
    SysinspectError,
    cfg::{mmconf::MasterConfig, select_config_path},
    logger::{self, STDOUTLogger},
};
use log::LevelFilter;
use std::{env, fs::File, sync::OnceLock};
use std::{path::PathBuf, process::exit};

static APPNAME: &str = "sysmaster";
static VERSION: &str = "0.4.0";
static LOGGER: OnceLock<logger::STDOUTLogger> = OnceLock::new();

fn start_master(cfg: MasterConfig) -> Result<(), SysinspectError> {
    tokio::runtime::Runtime::new()?.block_on(async {
        master::master(cfg).await?;
        Ok::<(), SysinspectError>(())
    })?;
    Ok(())
}

// Print help?
fn help(cli: &mut Command, params: ArgMatches) -> bool {
    if params.get_flag("help") {
        _ = &cli.print_long_help();
        return true;
    }

    // Print a global version?
    if params.get_flag("version") {
        println!("Version: {APPNAME} {VERSION}");
        return true;
    }

    false
}

fn main() -> Result<(), SysinspectError> {
    let mut cli = cli(VERSION, APPNAME);
    let params = cli.to_owned().get_matches();

    // Print help?
    if env::args().collect::<Vec<String>>().len() == 1 || *params.get_one::<bool>("help").unwrap() {
        cli.print_help()?;
        std::process::exit(1);
    }

    // Print version?
    if help(&mut cli, params.to_owned()) {
        std::process::exit(1);
    }

    // Setup logger
    if let Err(err) = log::set_logger(LOGGER.get_or_init(|| STDOUTLogger::new(params.get_flag("no-color")))).map(|()| {
        log::set_max_level(match params.get_count("debug") {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            2.. => LevelFilter::max(),
        })
    }) {
        println!("Error setting logger output: {err}");
    }

    // Get config
    let mut cfp = PathBuf::from(params.get_one::<String>("config").unwrap_or(&"".to_string()).to_owned());
    if !cfp.exists() {
        cfp = match select_config_path(None) {
            Ok(cfp) => {
                log::debug!("Reading config at {}", cfp.to_str().unwrap_or_default());
                cfp
            }
            Err(err) => {
                log::error!("{err}");
                std::process::exit(1);
            }
        };
    }
    let cfg = MasterConfig::new(cfp)?;

    // Mode
    if params.get_flag("start") {
        if let Err(err) = start_master(cfg) {
            log::error!("Error starting master: {err}");
        }
    } else if params.get_flag("stop") {
        log::info!("Stopping daemon");
        if let Err(err) = libsysinspect::util::sys::kill_process(cfg.pidfile(), Some(2)) {
            log::error!("Unable to stop sysmaster: {err}");
        }
        log::info!("Sysmaster is stopped");
    } else if params.get_flag("daemon") {
        log::info!("Starting daemon");
        let sout = match File::create(cfg.logfile_std()) {
            Ok(sout) => {
                log::info!("Opened main log file at {}", cfg.logfile_std().to_str().unwrap_or_default());
                sout
            }
            Err(err) => {
                log::error!("Unable to create main log file at {}: {err}, terminating", cfg.logfile_std().to_str().unwrap_or_default());
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
                if let Err(err) = start_master(cfg) {
                    log::error!("Error starting master: {err}");
                }
            }
            Err(e) => {
                log::error!("Error starting master in daemon mode: {e}");
                exit(1)
            }
        }
    }

    Ok(())
}
