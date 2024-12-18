mod clidef;
mod filedata;
mod minion;
mod proto;
mod rsa;

use clidef::cli;
use daemonize::Daemonize;
use libsysinspect::{
    cfg::{get_minion_config, mmconf::MinionConfig},
    logger, SysinspectError,
};
use log::LevelFilter;
use std::{env, fs::File, path::PathBuf, process::exit};

static APPNAME: &str = "sysminion";
static VERSION: &str = "0.3.0";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;

fn start_minion(cfg: MinionConfig, fp: Option<String>) -> Result<(), SysinspectError> {
    tokio::runtime::Runtime::new()?.block_on(async {
        minion::minion(cfg, fp).await?;
        Ok::<(), SysinspectError>(())
    })?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    let mut cli = cli(VERSION, APPNAME);
    if env::args().collect::<Vec<String>>().len() == 1 {
        cli.print_help()?;
        std::process::exit(1);
    }

    let params = cli.to_owned().get_matches();

    // Print help?
    if params.get_flag("help") {
        return cli.print_help();
    }

    // Print version?
    if params.get_flag("version") {
        println!("Version: {} {}", APPNAME, VERSION);
        return Ok(());
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

    // Config
    let cfg = match get_minion_config(Some(params.get_one::<String>("config").map_or("", |v| v))) {
        Ok(cfg) => cfg,
        Err(err) => {
            log::error!("Unable to find a Minion config: {err}");
            exit(1);
        }
    };

    // Start
    let fp = params.get_one::<String>("register").cloned();
    if params.get_flag("start") || fp.is_some() {
        if let Err(err) = start_minion(cfg, fp) {
            log::error!("Error starting minion: {err}");
        }
    } else if params.get_flag("daemon") {
        log::info!("Starting daemon");
        let sout = match File::create("/tmp/sysminion.log") {
            Ok(sout) => {
                log::info!("Opened main log file at {}", "sysminion.log");
                sout
            }
            Err(err) => {
                log::error!("Unable to create main log file at {}: {err}, terminating", "sysminion.log");
                exit(1);
            }
        };
        let serr = match File::create("/tmp/sysminion.err") {
            Ok(serr) => {
                log::info!("Opened error log file at {}", "sysminion.err");

                serr
            }
            Err(err) => {
                log::error!("Unable to create file at {}: {err}, terminating", "sysminion.err");
                exit(1);
            }
        };

        match Daemonize::new().pid_file("/tmp/sysminion.pid").stdout(sout).stderr(serr).start() {
            Ok(_) => {
                log::info!("Daemon started with PID file at {}", "/tmp/sysminion.pid");
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
        if let Err(err) = libsysinspect::util::sys::kill_process(PathBuf::from("/tmp/sysminion.pid"), Some(2)) {
            log::error!("Unable to stop sysminion: {err}");
        }
    }

    Ok(())
}
