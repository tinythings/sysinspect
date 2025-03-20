use clap::builder::styling;
use clap::{Arg, ArgAction, Command};
use colored::Colorize;

/// Define CLI arguments and styling
pub fn cli(version: &'static str, appname: &'static str) -> Command {
    let styles = styling::Styles::styled()
        .header(styling::AnsiColor::Yellow.on_default())
        .usage(styling::AnsiColor::Yellow.on_default())
        .literal(styling::AnsiColor::BrightGreen.on_default())
        .placeholder(styling::AnsiColor::BrightMagenta.on_default());

    Command::new(appname)
        .version(version)
        .about(format!("{} - {}", appname.bright_magenta().bold(), "is a master node to operate minion agents"))
        .override_usage(format!("{} [OPTIONS]", appname))

        // Module management
        .subcommand(Command::new("module").about("Add, remove or list modules").styles(styles.clone()).disable_help_flag(true)
            .arg(Arg::new("add").short('A').long("add").action(ArgAction::SetTrue).help("Add a module to the repository").conflicts_with_all(["remove", "list"]))
            .arg(Arg::new("remove").short('R').long("remove").action(ArgAction::SetTrue).help("Remove a module from the repository").conflicts_with_all(["add", "list"]))
            .arg(Arg::new("list").short('L').long("list").action(ArgAction::SetTrue).help("List all modules in the repository").conflicts_with_all(["add", "remove"]))

            .arg(Arg::new("name").short('n').long("name").required_unless_present("help").help("Module name"))
            .arg(Arg::new("arch").short('a').long("arch").help("Module architecture (x86, x64, arm, arm64, noarch)").default_value("noarch"))
            .arg(Arg::new("binary").short('b').long("binary").action(ArgAction::SetTrue).help("Add a binary module"))
            .arg(Arg::new("path").short('p').long("path").required_unless_present("help").help("Path to the module"))

            .arg(Arg::new("help").short('h').long("help").action(ArgAction::SetTrue).help("Display help on this command"))
    )

        // Config
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Alternative path to the config")
        )
        .arg(
            Arg::new("start")
                .long("start")
                .action(ArgAction::SetTrue)
                .conflicts_with("daemon")
                .help("Start master in foreground mode")
        )
        .arg(
            Arg::new("daemon")
                .long("daemon")
                .action(ArgAction::SetTrue)
                .conflicts_with("start")
                .help("Start master in daemon mode")
        )
        .arg(
            Arg::new("stop")
                .long("stop")
                .action(ArgAction::SetTrue)
                .help("Stop master if it is in daemon mode")
        )

        .next_help_heading("Info")
        .arg(
            Arg::new("status")
                .long("status")
                .action(ArgAction::SetTrue)
                .help("Show connected minions")
        )


        // Other
        .next_help_heading("Other")
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
}
