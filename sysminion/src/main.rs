mod clidef;
mod filedata;
mod minion;
mod proto;
mod rsa;

use clidef::cli;
use libsysinspect::{logger, SysinspectError};
use log::LevelFilter;
use std::env;

static APPNAME: &str = "sysminion";
static VERSION: &str = "0.3.0";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;

fn start_minion(cfp: Option<&String>, fp: Option<String>) -> Result<(), SysinspectError> {
    tokio::runtime::Runtime::new()?.block_on(async {
        minion::minion(cfp.map_or("", |v| v), fp).await?;
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

    // Start
    let fp = params.get_one::<String>("register").cloned();
    if *params.get_one::<bool>("start").unwrap_or(&false) || fp.is_some() {
        if let Err(err) = start_minion(params.get_one::<String>("config"), fp) {
            log::error!("Error starting minion: {err}");
        }
    }

    Ok(())
}
