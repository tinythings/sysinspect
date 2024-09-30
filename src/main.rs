use std::env;

mod clidef;
mod logger;
mod mcf;

static VERSION: &str = "0.1";
static LOGGER: logger::STDOUTLogger = logger::STDOUTLogger;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut cli = clidef::cli(&VERSION);

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
        return println!("{}", err.to_string());
    }

    /*
    let r = match libsysinspect::mdl::mspec::load(".") {
        Ok(m) => {
            println!("{:?}", m);
        }
        Err(err) => println!("Error: {}", err),
    };
    */
}
