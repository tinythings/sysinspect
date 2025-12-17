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
        .override_usage(format!("{appname} [OPTIONS]"))

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
                .short('s')
                .long("start")
                .conflicts_with("daemon")
                .action(ArgAction::SetTrue)
                .help("Start minion in foreground")
        )
        .arg(
            Arg::new("daemon")
                .short('b')
                .long("daemon")
                .alias("back")
                .alias("background")
                .conflicts_with("start")
                .action(ArgAction::SetTrue)
                .help("Start minion as a daemon in background")
        )
        .arg(
            Arg::new("stop")
                .short('k')
                .long("stop")
                .action(ArgAction::SetTrue)
                .help("Stop minion if runs as a daemon")
        )
        .arg(
            Arg::new("info")
                .short('i')
                .long("info")
                .action(ArgAction::SetTrue)
                .help("Display minion info")
        )

        .next_help_heading("Minion")
        .subcommand(Command::new("setup").about("Minion local setup").styles(styles.clone()).disable_help_flag(true)
            .arg(Arg::new("with-default-config").short('c').long("with-default-config").action(ArgAction::SetTrue).help("Create a default config file"))
            .arg(Arg::new("master-addr").short('a').long("master-addr").help("<IP>:[port] address of the master"))
            .arg(Arg::new("directory").short('d').long("directory").help("Alternative writable path that would contain everything at once. Otherwise default paths are used."))
            .arg(Arg::new("dry-run").short('n').long("dry-run").action(ArgAction::SetTrue).help("Do not apply anything, just check the setup."))
            .arg(Arg::new("help").short('h').long("help").action(ArgAction::SetTrue).help("Display help on this command"))
        )

        .subcommand(Command::new("module").about("Local module invocation").styles(styles.clone()).disable_help_flag(true)
            .arg(Arg::new("name").short('n').long("name").help("Module name to invoke"))
            .arg(Arg::new("args").short('a').long("args").action(ArgAction::Append).value_parser(clap::builder::ValueParser::new(|s: &str|
                {
                    let parts: Vec<&str> = s.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        Ok((parts[0].to_string(), parts[1].to_string()))
                    } else {
                        Err(String::from("Key-value argument pair, translates to --key=value. Specify this pair multiple times for multiple pairs."))
                    }
                })).help("Key-value argument pairs to pass to the module (format: key=value)"))
            .arg(Arg::new("opts").short('o').long("opts").num_args(1..).value_parser(clap::builder::ValueParser::new(|s: &str|
                {
                    Ok::<Vec<String>, String>(s.split(',').map(|item| item.trim().to_string()).collect())
                })).help("Options to pass to the module (comma-separated)"))
            .arg(Arg::new("help").short('h').long("help").action(ArgAction::SetTrue).help("Display help on this command"))
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
