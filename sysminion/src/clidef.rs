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
                .conflicts_with("daemon")
                .action(ArgAction::SetTrue)
                .help("Start minion in foreground")
        )
        .arg(
            Arg::new("daemon")
                .long("daemon")
                .conflicts_with("start")
                .action(ArgAction::SetTrue)
                .help("Start minion as a daemon")
        )
        .arg(
            Arg::new("stop")
                .long("stop")
                .action(ArgAction::SetTrue)
                .help("Stop minion if runs as a daemon")
        )

        .next_help_heading("Minion")
        .subcommand(Command::new("setup").about("Minion local setup").styles(styles.clone()).disable_help_flag(true)
            .arg(Arg::new("master-addr")
                .short('a')
                .long("master-addr")
                .help("<IP>:[port] address of the master")
                .required_unless_present("help")
            )
            .arg(Arg::new("directory")
                .short('d')
                .long("directory")
                .help("Alternative writable path that would contain everything at once. Otherwise default paths are used.")
            )
            .arg(Arg::new("dry-run")
                .short('n')
                .long("dry-run")
                .action(ArgAction::SetTrue)
                .help("Do not apply anything, just check the setup.")
            )
            .arg(Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::SetTrue)
                .help("Display help on this command")
            )
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
