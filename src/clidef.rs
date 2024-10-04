use clap::builder::styling;
use clap::{Arg, ArgAction, Command};
use colored::Colorize;

static APPNAME: &str = "sysinspect";

/// Define CLI arguments and styling
pub fn cli(version: &'static str) -> Command {
    let styles = styling::Styles::styled()
        .header(styling::AnsiColor::White.on_default() | styling::Effects::BOLD)
        .usage(styling::AnsiColor::White.on_default() | styling::Effects::BOLD)
        .literal(styling::AnsiColor::BrightCyan.on_default())
        .placeholder(styling::AnsiColor::Cyan.on_default());

    Command::new(APPNAME)
        .version(version)
        .about(format!("{} - {}", APPNAME, "is a tool for anomaly detection and root cause analysis in a known system"))
        .override_usage(format!("{} {} {}", APPNAME.bright_cyan(), "[OPTIONS]".cyan(), "[FILTERS]".cyan()))

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
        )

        // Other
        .next_help_heading("Other")
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .action(ArgAction::SetTrue)
                .help("Set debug mode for more verbose output."),
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
