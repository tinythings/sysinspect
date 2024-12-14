use clap::ArgMatches;
use colored::Colorize;
use libsysinspect::{
    cfg::{mmconf::MasterConfig, select_config_path},
    inspector::SysInspectRunner,
    logger,
    proto::query::{commands::CLUSTER_SHUTDOWN, SCHEME_COMMAND},
    reactor::handlers,
    traits::get_minion_traits,
    SysinspectError,
};
use log::LevelFilter;
use std::{env, fs::OpenOptions, io::Write};

mod clidef;

static VERSION: &str = "0.2.0";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;

/// Display event handlers
fn print_event_handlers() {
    handlers::registry::init_handlers();
    println!("{}", format!("Supported event handlers in {}:", clidef::APPNAME.bold()).yellow());
    for (i, h) in handlers::registry::get_handler_names().iter().enumerate() {
        println!("  {}. {}", i + 1, h);
    }
    println!();
}

/// Call master via FIFO
fn call_master_fifo(model: &str, query: &str, traits: Option<&String>, fifo: &str) -> Result<(), SysinspectError> {
    let payload = format!("{model};{query};{}\n", traits.unwrap_or(&"".to_string()));
    OpenOptions::new().write(true).open(fifo)?.write_all(payload.as_bytes())?;

    log::debug!("Message sent to the master via FIFO: {:?}", payload);
    Ok(())
}

/// Set logger
fn set_logger(p: &ArgMatches) {
    if let Err(err) = log::set_logger(&LOGGER).map(|()| {
        log::set_max_level(match p.get_count("debug") {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            2.. => LevelFilter::max(),
        })
    }) {
        println!("{}", err)
    }
}

/// Get configuration of the master
fn get_cfg(p: &ArgMatches) -> Result<MasterConfig, SysinspectError> {
    MasterConfig::new(select_config_path(p.get_one::<&str>("config").cloned())?)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut cli = clidef::cli(VERSION);

    if args.len() == 1 {
        return {
            cli.print_help().unwrap();
        };
    }

    // Our main params
    let params = cli.to_owned().get_matches();

    // Set logger
    set_logger(&params);

    // Print help?
    if *params.get_one::<bool>("help").unwrap() {
        return {
            cli.print_help().unwrap();
        };
    }

    // Print version?
    if *params.get_one::<bool>("version").unwrap() {
        return {
            println!("Version {}", VERSION);
        };
    }

    if *params.get_one::<bool>("list-handlers").unwrap_or(&false) {
        print_event_handlers();
        return;
    }

    // Get master config
    let cfg = match get_cfg(&params) {
        Ok(cfg) => cfg,
        Err(err) => {
            log::error!("Unable to get master configuration: {err}");
            std::process::exit(1);
        }
    };

    if let Some(model) = params.get_one::<String>("path") {
        let query = params.get_one::<String>("query");
        let traits = params.get_one::<String>("traits");
        if let Err(err) = call_master_fifo(model, query.unwrap_or(&"".to_string()), traits, &cfg.socket()) {
            log::error!("Cannot reach master: {err}");
        }
    } else if params.get_flag("shutdown") {
        if let Err(err) = call_master_fifo(&format!("{}{}", SCHEME_COMMAND, CLUSTER_SHUTDOWN), "*", None, &cfg.socket()) {
            log::error!("Cannot reach master: {err}");
        }
    } else if let Some(mpath) = params.get_one::<String>("model") {
        let mut sr = SysInspectRunner::new(None);
        sr.set_model_path(mpath);
        sr.set_state(params.get_one::<String>("state").cloned());
        sr.set_entities(clidef::split_by(&params, "entities", None));
        sr.set_checkbook_labels(clidef::split_by(&params, "labels", None));
        sr.set_traits(get_minion_traits(None));
        sr.start();
    }
}
