use clap::{ArgMatches, Command};
use colored::Colorize;
use libcommon::SysinspectError;
use libmodpak::{self, mpk::ModPakMetadata};
use libsysinspect::{
    cfg::{
        mmconf::{MasterConfig, MinionConfig},
        select_config_path,
    },
    console::{ConsoleQuery, ConsoleResponse, ConsoleSealed, build_console_query},
    context,
    inspector::SysInspectRunner,
    logger::{self, MemoryLogger, STDOUTLogger},
    reactor::handlers,
    traits::get_minion_traits,
};
use libsysproto::query::SCHEME_COMMAND;
use libsysproto::query::commands::{
    CLUSTER_ONLINE_MINIONS, CLUSTER_PROFILE, CLUSTER_REMOVE_MINION, CLUSTER_SHUTDOWN, CLUSTER_SYNC, CLUSTER_TRAITS_UPDATE,
};
use log::LevelFilter;
use serde_json::json;
use std::{
    env,
    io::ErrorKind,
    path::PathBuf,
    process::exit,
    sync::{Mutex, OnceLock},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

mod clidef;
mod ui;

static VERSION: &str = "0.4.0";
static LOGGER: OnceLock<logger::STDOUTLogger> = OnceLock::new();
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

async fn call_master_console(
    cfg: &MasterConfig, model: &str, query: &str, traits: Option<&String>, mid: Option<&str>, context: Option<&String>,
) -> Result<ConsoleResponse, SysinspectError> {
    let request = ConsoleQuery {
        model: model.to_string(),
        query: query.to_string(),
        traits: traits.cloned().unwrap_or_default(),
        mid: mid.unwrap_or_default().to_string(),
        context: context.cloned().unwrap_or_default(),
    };
    let (envelope, key) = build_console_query(&cfg.root_dir(), cfg, &request)?;
    let mut stream = TcpStream::connect(cfg.console_connect_addr()).await?;
    stream.write_all(format!("{}\n", serde_json::to_string(&envelope)?).as_bytes()).await?;

    let mut reader = BufReader::new(stream);
    let mut reply = String::new();
    reader.read_line(&mut reply).await?;
    let response: ConsoleResponse = match serde_json::from_str::<ConsoleSealed>(reply.trim()) {
        Ok(sealed) => sealed.open(&key)?,
        Err(_) => serde_json::from_str(reply.trim())?,
    };
    if !response.ok {
        return Err(SysinspectError::MasterGeneralError(response.message));
    }
    Ok(response)
}

fn traits_update_context(am: &ArgMatches) -> Result<Option<String>, SysinspectError> {
    if let Some(setv) = am.get_one::<String>("set") {
        let traits = context::get_context(setv)
            .ok_or_else(|| SysinspectError::InvalidQuery("Trait values must be in key:value format".to_string()))?;
        return Ok(Some(json!({"op": "set", "traits": traits}).to_string()));
    }

    if let Some(keys) = am.get_one::<String>("unset") {
        return Ok(Some(json!({
            "op": "unset",
            "traits": context::get_context_keys(keys).into_iter().map(|key| (key, serde_json::Value::Null)).collect::<serde_json::Map<String, serde_json::Value>>()
        })
        .to_string()));
    }

    if am.get_flag("reset") {
        return Ok(Some(json!({"op": "reset", "traits": {}}).to_string()));
    }

    Err(SysinspectError::InvalidQuery("Specify one of --set, --unset, or --reset".to_string()))
}

fn profile_update_context(am: &ArgMatches) -> Result<Option<String>, SysinspectError> {
    let invalid_name = |name: &str| name.chars().any(|c| ['*', '?', '[', ']'].contains(&c));
    if am.get_flag("new") {
        if am.get_one::<String>("name").is_none() {
            return Err(SysinspectError::InvalidQuery("Specify --name for --new".to_string()));
        }
        if invalid_name(am.get_one::<String>("name").unwrap()) {
            return Err(SysinspectError::InvalidQuery("Profile names for --new must be exact names, not glob patterns".to_string()));
        }
        return Ok(Some(json!({"op": "new", "name": am.get_one::<String>("name").cloned().unwrap_or_default()}).to_string()));
    }

    if am.get_flag("delete") {
        if am.get_one::<String>("name").is_none() {
            return Err(SysinspectError::InvalidQuery("Specify --name for --delete".to_string()));
        }
        if invalid_name(am.get_one::<String>("name").unwrap()) {
            return Err(SysinspectError::InvalidQuery("Profile names for --delete must be exact names, not glob patterns".to_string()));
        }
        return Ok(Some(json!({"op": "delete", "name": am.get_one::<String>("name").cloned().unwrap_or_default()}).to_string()));
    }

    if am.get_flag("list") {
        return Ok(Some(
            json!({"op": "list", "name": am.get_one::<String>("name").cloned().unwrap_or_default(), "library": am.get_flag("lib")}).to_string(),
        ));
    }

    if am.get_flag("show") {
        if am.get_one::<String>("name").is_none() {
            return Err(SysinspectError::InvalidQuery("Specify --name for --show".to_string()));
        }
        if invalid_name(am.get_one::<String>("name").unwrap()) {
            return Err(SysinspectError::InvalidQuery("Profile names for --show must be exact names, not glob patterns".to_string()));
        }
        return Ok(Some(json!({"op": "show", "name": am.get_one::<String>("name").cloned().unwrap_or_default()}).to_string()));
    }

    if am.get_flag("add") || am.get_flag("remove") {
        if am.get_one::<String>("name").is_none() || am.get_one::<String>("match").is_none() {
            return Err(SysinspectError::InvalidQuery("Specify both --name and --match for profile selector updates".to_string()));
        }
        if invalid_name(am.get_one::<String>("name").unwrap()) {
            return Err(SysinspectError::InvalidQuery("Profile names for selector updates must be exact names, not glob patterns".to_string()));
        }
        if clidef::split_by(am, "match", None).is_empty() {
            return Err(SysinspectError::InvalidQuery("At least one selector is required in --match".to_string()));
        }
        return Ok(Some(
            json!({
                "op": if am.get_flag("add") { "add" } else { "remove" },
                "name": am.get_one::<String>("name").cloned().unwrap_or_default(),
                "matches": clidef::split_by(am, "match", None),
                "library": am.get_flag("lib"),
            })
            .to_string(),
        ));
    }

    if am.get_one::<String>("tag").is_some() || am.get_one::<String>("untag").is_some() {
        if clidef::split_by(am, if am.get_one::<String>("tag").is_some() { "tag" } else { "untag" }, None).is_empty() {
            return Err(SysinspectError::InvalidQuery("Specify at least one profile name for --tag or --untag".to_string()));
        }
        return Ok(Some(
            json!({
                "op": if am.get_one::<String>("tag").is_some() { "tag" } else { "untag" },
                "profiles": clidef::split_by(am, if am.get_one::<String>("tag").is_some() { "tag" } else { "untag" }, None),
            })
            .to_string(),
        ));
    }

    Err(SysinspectError::InvalidQuery("Specify one profile operation".to_string()))
}

/// Set logger
fn set_logger(p: &ArgMatches) {
    let log: &'static dyn log::Log = if *p.get_one::<bool>("ui").unwrap_or(&false) {
        &MEM_LOGGER as &'static dyn log::Log
    } else {
        LOGGER.get_or_init(STDOUTLogger::default) as &'static dyn log::Log
    };

    if let Err(err) = log::set_logger(log).map(|()| {
        log::set_max_level(match p.get_count("debug") {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            2.. => LevelFilter::max(),
        })
    }) {
        println!("{err}")
    }
}

/// Get configuration of the master
fn get_cfg(p: &ArgMatches) -> Result<MasterConfig, SysinspectError> {
    MasterConfig::new(select_config_path(p.get_one::<&str>("config").cloned())?)
}

// Print help?
fn help(cli: &mut Command, params: &ArgMatches) -> bool {
    if let Some(sub) = params.subcommand_matches("module")
        && sub.get_flag("help")
    {
        if let Some(s_cli) = cli.find_subcommand_mut("module") {
            _ = s_cli.print_help();
            return true;
        }
        return false;
    }
    if let Some(sub) = params.subcommand_matches("traits")
        && sub.get_flag("help")
    {
        if let Some(s_cli) = cli.find_subcommand_mut("traits") {
            _ = s_cli.print_help();
            return true;
        }
        return false;
    }
    if let Some(sub) = params.subcommand_matches("profile")
        && sub.get_flag("help")
    {
        if let Some(s_cli) = cli.find_subcommand_mut("profile") {
            _ = s_cli.print_help();
            return true;
        }
        return false;
    }
    if params.get_flag("help") {
        _ = &cli.print_long_help();
        return true;
    }

    // Print a global version?
    if params.get_flag("version") {
        println!("Version: {VERSION}");
        return true;
    }

    false
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
    if help(&mut cli, &params) {
        std::process::exit(0);
    }

    // Get master config
    let cfg = match get_cfg(&params) {
        Ok(cfg) => cfg,
        Err(err) => {
            log::error!("Unable to get master configuration: {err}");
            std::process::exit(1);
        }
    };

    if let Some(sub) = params.subcommand_matches("module") {
        let mut repo = match libmodpak::SysInspectModPak::new(cfg.get_mod_repo_root()) {
            Ok(repo) => repo,
            Err(err) => {
                if let SysinspectError::IoErr(err) = &err
                    && err.kind() == ErrorKind::NotFound
                {
                    log::error!("No module repository found. Create one, perhaps?..");
                    exit(1);
                }
                log::error!("Unable to open module repository: {err}");
                exit(1);
            }
        };

        if sub.get_flag("add") {
            if sub.get_flag("lib") {
                log::info!("Processing library in {}", cfg.get_mod_repo_root().to_str().unwrap_or_default());
                if let Err(err) = repo.add_library(PathBuf::from(sub.get_one::<String>("path").unwrap_or(&"".to_string()))) {
                    log::error!("Failed to add library: {err}");
                    exit(1);
                }
            } else {
                log::info!("Processing modules in {}", cfg.get_mod_repo_root().to_str().unwrap_or_default());
                let meta = match ModPakMetadata::from_cli_matches(sub) {
                    Ok(m) => m,
                    Err(err) => {
                        log::error!("{err}");
                        exit(1);
                    }
                };
                if let Err(err) = repo.add_module(meta) {
                    log::error!("Failed to add module: {err}");
                    exit(1);
                }
            }
        } else if sub.get_flag("list") {
            if sub.get_flag("lib") {
                repo.list_libraries(sub.get_one::<String>("match").map(String::as_str)).unwrap_or_else(|err| {
                    log::error!("Failed to list libraries: {err}");
                    exit(1);
                });
            } else {
                repo.list_modules().unwrap_or_else(|err| {
                    log::error!("Failed to list modules: {err}");
                    exit(1);
                });
            }
        } else if sub.get_flag("info") {
            if let Err(err) = repo.module_info(sub.get_one::<String>("name").unwrap_or(&"".to_string())) {
                log::error!("Failed to get module info: {err}");
                exit(1);
            }
        } else if sub.get_flag("remove") {
            if sub.get_one::<String>("name").is_none() {
                log::error!("Specify the module or library name ({}).", "--name".bright_yellow());
                exit(1);
            }
            if sub.get_flag("lib") {
                let names: Vec<String> = sub
                    .get_one::<String>("name")
                    .unwrap_or(&String::new())
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if names.is_empty() {
                    log::error!("No library names provided for removal.");
                    exit(1);
                }
                repo.remove_library(names).unwrap_or_else(|err| {
                    log::error!("Failed to remove libraries: {err}");
                    exit(1);
                });
            } else {
                let s = "".to_string();
                if let Err(err) = repo.remove_module(sub.get_one::<String>("name").unwrap_or(&s).split(',').map(|s| s.trim()).collect()) {
                    log::error!("Failed to remove modules: {err}");
                    exit(1);
                }
            }
        };
        exit(0)
    }

    if let Some(sub) = params.subcommand_matches("traits") {
        let target_id = sub.get_one::<String>("id").map(String::as_str);
        let target_query = sub
            .get_one::<String>("query")
            .or_else(|| sub.get_one::<String>("query-pos"))
            .map(String::as_str)
            .unwrap_or("*");
        let target_traits = sub.get_one::<String>("select-traits");
        let scheme = format!("{SCHEME_COMMAND}{CLUSTER_TRAITS_UPDATE}");

        let context = match traits_update_context(sub) {
            Ok(ctx) => ctx,
            Err(err) => {
                log::error!("{err}");
                exit(1);
            }
        };

        if let Err(err) = call_master_console(&cfg, &scheme, target_query, target_traits, target_id, context.as_ref()).await {
            log::error!("Cannot reach master: {err}");
        }
        exit(0);
    }

    if let Some(sub) = params.subcommand_matches("profile") {
        let target_id = sub.get_one::<String>("id").map(String::as_str);
        let target_query = sub
            .get_one::<String>("query")
            .or_else(|| sub.get_one::<String>("query-pos"))
            .map(String::as_str)
            .unwrap_or("*");
        let target_traits = sub.get_one::<String>("select-traits");
        let context = match profile_update_context(sub) {
            Ok(ctx) => ctx,
            Err(err) => {
                log::error!("{err}");
                exit(1);
            }
        };

        match call_master_console(&cfg, &format!("{SCHEME_COMMAND}{CLUSTER_PROFILE}"), target_query, target_traits, target_id, context.as_ref()).await {
            Ok(resp) => {
                if !resp.message.is_empty() {
                    println!("{}", resp.message);
                }
            }
            Err(err) => log::error!("Cannot reach master: {err}"),
        }
        exit(0);
    }

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
        let context = params.get_one::<String>("context");
        if let Err(err) = call_master_console(&cfg, model, query.unwrap_or(&"".to_string()), traits, None, context).await {
            log::error!("Cannot reach master: {err}");
        }
    } else if params.get_flag("shutdown") {
        if let Err(err) = call_master_console(&cfg, &format!("{SCHEME_COMMAND}{CLUSTER_SHUTDOWN}"), "*", None, None, None).await {
            log::error!("Cannot reach master: {err}");
        }
    } else if params.get_flag("sync") {
        if let Err(err) = call_master_console(&cfg, &format!("{SCHEME_COMMAND}{CLUSTER_SYNC}"), "*", None, None, None).await {
            log::error!("Cannot reach master: {err}");
        }
    } else if let Some(mid) = params.get_one::<String>("unregister") {
        if let Err(err) = call_master_console(&cfg, &format!("{SCHEME_COMMAND}{CLUSTER_REMOVE_MINION}"), "", None, Some(mid), None).await {
            log::error!("Cannot reach master: {err}");
        }
    } else if params.get_flag("online") {
        match call_master_console(&cfg, &format!("{SCHEME_COMMAND}{CLUSTER_ONLINE_MINIONS}"), "", None, None, None).await {
            Ok(response) if !response.message.is_empty() => println!("{}", response.message),
            Ok(_) => {}
            Err(err) => log::error!("Cannot reach master: {err}"),
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
