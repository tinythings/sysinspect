use clap::{Arg, ArgAction, Command};
use clap::{ArgMatches, builder::styling};
use colored::Colorize;

pub static APPNAME: &str = "sysinspect";

/// Define CLI arguments and styling
pub fn cli(version: &'static str) -> Command {
    let styles = styling::Styles::styled()
        .header(styling::AnsiColor::Yellow.on_default())
        .usage(styling::AnsiColor::Yellow.on_default())
        .literal(styling::AnsiColor::BrightGreen.on_default())
        .placeholder(styling::AnsiColor::BrightMagenta.on_default());

    Command::new(APPNAME)
        .version(version)
        .about(format!("{} - {}", APPNAME.bright_magenta().bold(), "is a tool for anomaly detection and root cause analysis in a known system"))
        .override_usage(format!("{} [OPTIONS] [FILTERS]", APPNAME))

        // Module management
        .subcommand(Command::new("module").about("Add, remove or list modules").styles(styles.clone()).disable_help_flag(true)
            .arg(Arg::new("add").short('A').long("add").action(ArgAction::SetTrue).help("Add a module to the repository").conflicts_with_all(["remove", "list"]))
            .arg(Arg::new("remove").short('R').long("remove").action(ArgAction::SetTrue).help("Remove a module from the repository").conflicts_with_all(["add", "list"]))
            .arg(Arg::new("list").short('L').long("list").action(ArgAction::SetTrue).help("List all modules in the repository").conflicts_with_all(["add", "remove"]))

            .arg(Arg::new("lib").short('l').long("lib").action(ArgAction::SetTrue).help("Module is a library (usually Python scripts)").conflicts_with_all(["list"]))

            .arg(Arg::new("name").short('n').long("name").required_unless_present_any(["help", "list", "lib"]).help("Module name"))
            .arg(Arg::new("path").short('p').long("path").required_unless_present_any(["help", "list", "remove"]).help("Path to the module (or library)"))
            .arg(Arg::new("descr").short('d').long("descr").required_unless_present_any(["help", "list", "lib", "remove"]).help("Description of the module"))
            .arg(Arg::new("arch").short('a').long("arch").help("Module architecture (x86, x64, arm, arm64, noarch)").default_value("noarch"))

            .arg(Arg::new("help").short('h').long("help").action(ArgAction::SetTrue).help("Display help on this command"))
        )

        // Sysinspect
        .next_help_heading("Main")
        .arg(
            Arg::new("path")
                .help("Specify model path that needs to be requested")
                .required(false)
                .index(1)
        )
        .arg(
            Arg::new("query")
                .help("Minions to query")
                .required(false)
                .index(2)
        )
        .arg(
            Arg::new("traits")
                .short('t')
                .long("traits")
                .help("Specify traits to select remote systems")
        )

        // Local
        .next_help_heading("Local")

        // Config
        .arg(
            Arg::new("model")
                .short('m')
                .long("model")
                .help("System description model")
        )
        .arg(
            Arg::new("labels")
                .short('l')
                .long("labels")
                .help("Select only specific labels from the checkbook (comma-separated)")
                .conflicts_with_all(["entities"])
        )
        .arg(
            Arg::new("entities")
                .short('e')
                .long("entities")
                .help("Select only specific entities from the inventory (comma-separated)")
                .conflicts_with_all(["labels"])
        )
        .arg(
            Arg::new("state")
                .short('s')
                .long("state")
                .help("Specify a state to be processed. If none specified, default is taken ($)")
        )

        // Cluster
        .next_help_heading("Cluster")
        .arg(
            Arg::new("ui")
            .short('u')
            .long("ui")
            .action(ArgAction::SetTrue)
            .help("Run terminal user interface app (TUI) for the review of the results")
        )
        .arg(
            Arg::new("unregister")
                .long("unregister")
                .help("Unregister a minion by its System Id. New registration will be required.")
        )
        .arg(
            Arg::new("shutdown")
                .long("shutdown")
                .action(ArgAction::SetTrue)
                .help(format!("Notify the running master to shutdown the {}, be careful! :)", "entire cluster".bright_red()))
        )

        .next_help_heading("Model")
        .arg(
            Arg::new("list-handlers")
                .long("list-handlers")
                .action(ArgAction::SetTrue)
                .help("List available event handler Ids")
        )


        // Other
        .next_help_heading("Other")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Specify alternative configuration")
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .action(ArgAction::Count)
                .help("Set debug mode for more verbose output. Increase this flag for more verbosity."),
        )
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::SetTrue)
                .help("Display help"),
        )
        .arg(
            Arg::new("version")
                .short('v')
                .long("version")
                .action(ArgAction::SetTrue)
                .help("Get current version."),
        )
        .disable_help_flag(true) // Otherwise it is displayed in a wrong position
        .disable_version_flag(true)
        .disable_colored_help(false)
        .styles(styles)
        .after_help("NOTE: This tool is in very early development.
      If it doesn't work for you, please fill a bug report here:
      https://github.com/tinythings/sysinspect/issues\n".bright_yellow().to_string())
}

/// Parse comma-separated values
pub fn split_by(am: &ArgMatches, id: &str, sep: Option<char>) -> Vec<String> {
    let fsep: char;
    if let Some(sep) = sep {
        fsep = sep;
    } else {
        fsep = ',';
    }

    am.get_one::<String>(id)
        .unwrap_or(&"".to_string())
        .split(fsep)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<String>>()
}
