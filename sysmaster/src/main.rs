mod clidef;
mod dataserv;
mod master;
mod registry;
mod rmt;

use clidef::cli;
use daemonize::Daemonize;
use libsysinspect::{
    cfg::{mmconf::MasterConfig, select_config_path},
    logger, SysinspectError,
};
use log::LevelFilter;
use std::{env, fs::File};
use std::{path::PathBuf, process::exit};

static APPNAME: &str = "sysmaster";
static VERSION: &str = "0.0.1";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;

fn start_master(cfg: MasterConfig) -> Result<(), SysinspectError> {
    tokio::runtime::Runtime::new()?.block_on(async {
        master::master(cfg).await?;
        Ok::<(), SysinspectError>(())
    })?;
    Ok(())
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
    if *params.get_one::<bool>("version").unwrap() {
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

    // Get config
    let mut cfp = PathBuf::from(params.get_one::<String>("config").unwrap_or(&"".to_string()).to_owned());
    if !cfp.exists() {
        cfp = match select_config_path(None) {
            Ok(cfp) => {
                log::debug!("Reading config at {}", cfp.to_str().unwrap_or_default());
                cfp
            }
            Err(err) => {
                log::error!("{}", err);
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
    } else if params.get_flag("daemon") {
        log::info!("Starting daemon");
        let pid = "/tmp/sysmaster.pid";
        let sout = File::create("/tmp/sysmaster.out").unwrap();
        let serr = File::create("/tmp/sysmaster.err").unwrap();

        match Daemonize::new().pid_file(pid).stdout(sout).stderr(serr).start() {
            Ok(_) => {
                log::info!("Daemon started successfully.");
                if let Err(err) = start_master(cfg) {
                    log::error!("Error starting master: {err}");
                }
            }
            Err(e) => {
                log::error!("Error daemonizing: {}", e);
                exit(1);
            }
        }
    }

    Ok(())
}
