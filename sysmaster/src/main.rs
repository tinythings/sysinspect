mod clidef;
mod master;
mod registry;
mod rmt;

use clidef::cli;
use libsysinspect::{
    cfg::{mmconf::MasterConfig, select_config},
    logger, SysinspectError,
};
use log::LevelFilter;
use rmt::send_message;
use std::{env, path::PathBuf};

static APPNAME: &str = "sysmaster";
static VERSION: &str = "0.0.1";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;

#[tokio::main]
async fn main() -> Result<(), SysinspectError> {
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
        cfp = match select_config(None) {
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
    let query = params.get_one::<String>("query").unwrap_or(&"".to_string()).to_owned();
    if *params.get_one::<bool>("start").unwrap() {
        master::master(cfg).await?;
    } else if !query.is_empty() {
        log::info!("Query: {}", query);
        send_message(&query, &cfg.socket()).await?
    }

    Ok(())
}
