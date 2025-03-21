use clap::ArgMatches;
use colored::Colorize;
use libsysinspect::{
    SysinspectError,
    cfg::{
        mmconf::{MasterConfig, MinionConfig},
        select_config_path,
    },
    inspector::SysInspectRunner,
    logger::{self, MemoryLogger},
    proto::query::{
        SCHEME_COMMAND,
        commands::{CLUSTER_REMOVE_MINION, CLUSTER_SHUTDOWN},
    },
    reactor::handlers,
    traits::get_minion_traits,
};
use log::LevelFilter;
use std::{
    env,
    fs::OpenOptions,
    io::{ErrorKind, Write},
    sync::Mutex,
};

mod clidef;
mod ui;

static VERSION: &str = "0.4.0";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;
static MEM_LOGGER: MemoryLogger = MemoryLogger { messages: Mutex::new(Vec::new()) };

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
fn call_master_fifo(
    model: &str, query: &str, traits: Option<&String>, mid: Option<&str>, fifo: &str,
) -> Result<(), SysinspectError> {
    let payload = format!("{model};{query};{};{}\n", traits.unwrap_or(&"".to_string()), mid.unwrap_or_default());
    OpenOptions::new().write(true).open(fifo)?.write_all(payload.as_bytes())?;

    log::debug!("Message sent to the master via FIFO: {:?}", payload);
    Ok(())
}

/// Set logger
fn set_logger(p: &ArgMatches) {
    let log: &'static dyn log::Log = if *p.get_one::<bool>("ui").unwrap_or(&false) {
        &MEM_LOGGER as &'static dyn log::Log
    } else {
        &LOGGER as &'static dyn log::Log
    };

    if let Err(err) = log::set_logger(log).map(|()| {
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

#[tokio::main]
async fn main() {
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

    // Get master config
    let cfg = match get_cfg(&params) {
        Ok(cfg) => cfg,
        Err(err) => {
            log::error!("Unable to get master configuration: {err}");
            std::process::exit(1);
        }
    };

    if *params.get_one::<bool>("list-handlers").unwrap_or(&false) {
        print_event_handlers();
        return;
    } else if *params.get_one::<bool>("ui").unwrap_or(&false) {
        if let Err(err) = ui::run(cfg).await {
            let x = err.kind();
            if x == ErrorKind::InvalidData {
                println!(
                    "Can't start the UI: {}.\nIs {} running and reachable?\n",
                    err.to_string().bright_red(),
                    "SysInspect Master".bright_yellow()
                );
            } else {
                println!("Unexpected error: {}", err.to_string().bright_red())
            }
        }
        return;
    }

    if let Some(model) = params.get_one::<String>("path") {
        let query = params.get_one::<String>("query");
        let traits = params.get_one::<String>("traits");
        if let Err(err) = call_master_fifo(model, query.unwrap_or(&"".to_string()), traits, None, &cfg.socket()) {
            log::error!("Cannot reach master: {err}");
        }
    } else if params.get_flag("shutdown") {
        if let Err(err) = call_master_fifo(&format!("{}{}", SCHEME_COMMAND, CLUSTER_SHUTDOWN), "*", None, None, &cfg.socket()) {
            log::error!("Cannot reach master: {err}");
        }
    } else if let Some(mid) = params.get_one::<String>("unregister") {
        if let Err(err) =
            call_master_fifo(&format!("{}{}", SCHEME_COMMAND, CLUSTER_REMOVE_MINION), "", None, Some(mid), &cfg.socket())
        {
            log::error!("Cannot reach master: {err}");
        }
    } else if let Some(mpath) = params.get_one::<String>("model") {
        let mut sr = SysInspectRunner::new(&MinionConfig::default());
        sr.set_model_path(mpath);
        sr.set_state(params.get_one::<String>("state").cloned());
        sr.set_entities(clidef::split_by(&params, "entities", None));
        sr.set_checkbook_labels(clidef::split_by(&params, "labels", None));
        sr.set_traits(get_minion_traits(None));

        sr.start().await;
    }
}
