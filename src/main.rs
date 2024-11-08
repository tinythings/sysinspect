use colored::Colorize;
use inspector::SysInspectRunner;
use libsysinspect::{logger, reactor::handlers};
use log::LevelFilter;
use std::env;

mod clidef;
mod mcf;

static VERSION: &str = "0.2.0";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;

/// Display event handlers
pub fn print_event_handlers() {
    handlers::registry::init_handlers();
    println!("{}", format!("Supported event handlers in {}:", clidef::APPNAME.bold()).yellow());
    for (i, h) in handlers::registry::get_handler_names().iter().enumerate() {
        println!("  {}. {}", i + 1, h);
    }
    println!();
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

    // Setup logger
    if let Err(err) = log::set_logger(&LOGGER).map(|()| {
        log::set_max_level(match params.get_count("debug") {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            2.. => LevelFilter::max(),
        })
    }) {
        return println!("{}", err);
    }

    if let Some(mpath) = params.get_one::<String>("model") {
        let mut sr = SysInspectRunner::new();
        sr.set_model_path(mpath);
        sr.set_state(params.get_one::<String>("state").cloned());
        sr.set_entities(clidef::split_by(&params, "entities", None));
        sr.set_checkbook_labels(clidef::split_by(&params, "labels", None));
        sr.start();
    }
}
