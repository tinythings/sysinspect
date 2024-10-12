use clap::{builder::styling, ArgMatches};
use clap::{Arg, ArgAction, Command};
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

        .next_help_heading("Info")
        .arg(
            Arg::new("list-handlers")
                .long("list-handlers")
                .action(ArgAction::SetTrue)
                .help("List available event handler Ids")
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
