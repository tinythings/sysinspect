use libsysinspect::logger;
use std::env;

mod clidef;
mod mcf;

static VERSION: &str = "0.1";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;

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

    // Setup logger
    if let Err(err) = log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(if params.get_flag("debug") { log::LevelFilter::Trace } else { log::LevelFilter::Info }))
    {
        return println!("{}", err);
    }

    if let Some(mpath) = params.get_one::<String>("model") {
        match libsysinspect::mdl::mspec::load(mpath) {
            Ok(spec) => {
                log::debug!("Initalising inspector");
                match libsysinspect::intp::inspector::SysInspector::new(spec) {
                    Ok(isp) => {
                        // XXX: Move all this elsewhere
                        //let ar = isp.actions_by_relations(clidef::split_by(&params, "labels", None)).unwrap();
                        match isp.actions_by_entities(
                            clidef::split_by(&params, "entities", None),
                            params.get_one::<String>("state").cloned(),
                        ) {
                            Ok(actions) => {
                                for ac in actions {
                                    ac.run();
                                }
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
