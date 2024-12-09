mod clidef;
mod filedata;
mod minion;
mod proto;
mod rsa;

use clidef::cli;
use libsysinspect::logger;
use log::LevelFilter;
use std::env;

static APPNAME: &str = "sysminion";
static VERSION: &str = "0.0.1";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut cli = cli(VERSION, APPNAME);
    if env::args().collect::<Vec<String>>().len() == 1 {
        cli.print_help()?;
        std::process::exit(1);
    }

    let params = cli.to_owned().get_matches();

    // Print help?
    if *params.get_one::<bool>("help").unwrap() {
        return cli.print_help();
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

    // Start
    let fp = params.get_one::<String>("register").cloned();
    if *params.get_one::<bool>("start").unwrap_or(&false) || fp.is_some() {
        let cfp = params.get_one::<String>("config");
        if let Err(err) = minion::minion(cfp.map_or("", |v| v), fp).await {
            log::error!("Unable to start minion: {}", err);
            return Ok(());
        }
    }

    Ok(())
}
