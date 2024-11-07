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
        .about(format!("{} - {}", appname.bright_magenta().bold(), "is an agent client on a remote device"))
        .override_usage(format!("{} [OPTIONS]", appname))

        // Config
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Alternative path to the config")
        )
        .arg(
            Arg::new("register")
                .short('r')
                .long("register")
                .conflicts_with("start")
                .help("Register to the master by its fingerprint") // XXX: This must be a key fingerprint in a future
        )
        .arg(
            Arg::new("start")
                .long("start")
                .action(ArgAction::SetTrue)
                .help("Start minion")
        )

        .next_help_heading("Info")
        .arg(
            Arg::new("status")
                .long("status")
                .action(ArgAction::SetTrue)
                .help("Show minion status")
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
