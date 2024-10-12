use colored::Colorize;
use libsysinspect::{
    intp::actproc::response::ActionResponse,
    logger,
    reactor::{evtproc::EventProcessor, handlers},
};
use log::LevelFilter;
use std::env;

mod clidef;
mod mcf;

static VERSION: &str = "0.0.1";
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
        match libsysinspect::mdescr::mspec::load(mpath) {
            Ok(spec) => {
                log::debug!("Initalising inspector");
                match libsysinspect::intp::inspector::SysInspector::new(spec) {
                    Ok(isp) => {
                        // Setup event processor
                        let mut evtproc = EventProcessor::new().set_config(isp.cfg());

                        // XXX: Move all this elsewhere
                        //let ar = isp.actions_by_relations(clidef::split_by(&params, "labels", None)).unwrap();

                        match isp.actions_by_entities(
                            clidef::split_by(&params, "entities", None),
                            params.get_one::<String>("state").cloned(),
                        ) {
                            Ok(actions) => {
                                for ac in actions {
                                    match ac.run() {
                                        Ok(response) => {
                                            let response = response.unwrap_or(ActionResponse::default());
                                            evtproc.receiver().register(response.eid().to_owned(), response);
                                        }
                                        Err(err) => {
                                            log::error!("{err}")
                                        }
                                    }
                                }
                                evtproc.process();
                            }
                            Err(err) => {
                                log::error!("{}", err);
                            }
                        }
                    }
                    Err(err) => log::error!("{err}"),
                }
                log::debug!("Done");
            }
            Err(err) => println!("Error: {}", err),
        };
    }
}
