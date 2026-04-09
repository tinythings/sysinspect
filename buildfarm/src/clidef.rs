use clap::{Arg, ArgAction, ColorChoice, Command};
use clap::builder::styling;
use clap::ArgMatches;
use colored::Colorize;

pub static APPNAME: &str = "buildfarm";
pub static VERSION: &str = "0.1.0";

pub fn cli() -> Command {
    Command::new(APPNAME)
        .version(VERSION)
        .about(format!(
            "{} - {}",
            APPNAME.bright_magenta().bold(),
            "is a buildfarm TUI runner for local and remote build targets"
        ))
        .override_usage(format!("{APPNAME} [OPTIONS] <COMMAND>"))
        .subcommand(
            Command::new("init")
                .about("Validate and initialise buildfarm targets from BUILDFARM_CONFIG")
                .styles(styles()),
        )
        .subcommand(
            Command::new("run")
                .about("Run one buildfarm-aware make entry across configured targets")
                .styles(styles())
                .arg(
                    Arg::new("entry")
                        .help("Build entry to run, such as dev, release, modules, or test")
                        .required(true)
                        .index(1),
                ),
        )
        .next_help_heading("Other")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Specify an alternative buildfarm config instead of BUILDFARM_CONFIG"),
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .action(ArgAction::Count)
                .help("Enable debug mode for more verbose output. Increase this flag for greater verbosity."),
        )
        .color(ColorChoice::Always)
        .styles(styles())
}

pub fn entry(am: &ArgMatches) -> String {
    am.subcommand_matches("run")
        .and_then(|sub| sub.get_one::<String>("entry"))
        .cloned()
        .unwrap_or_default()
}

fn styles() -> styling::Styles {
    styling::Styles::styled()
        .header(styling::AnsiColor::Yellow.on_default())
        .usage(styling::AnsiColor::Yellow.on_default())
        .literal(styling::AnsiColor::BrightGreen.on_default())
        .placeholder(styling::AnsiColor::BrightMagenta.on_default())
}
