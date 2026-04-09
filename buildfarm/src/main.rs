mod clidef;
mod model;
mod runner;
#[cfg(test)]
mod model_ut;
#[cfg(test)]
mod runner_ut;

use std::{env, fs, process};

use clap::ArgMatches;

use model::BuildfarmConfig;

struct App;

impl App {
    fn run() -> ! {
        process::exit(Self::command().run());
    }

    fn command() -> Command {
        Command::from_matches(clidef::cli().get_matches())
    }
}

enum Command {
    Init,
    Run(String),
}

impl Command {
    fn from_matches(am: ArgMatches) -> Self {
        match am.subcommand_name() {
            Some("init") => Self::Init,
            Some("run") => Self::Run(clidef::entry(&am)),
            _ => Self::usage(),
        }
    }

    fn run(&self) -> i32 {
        match self {
            Self::Init => self.init(),
            Self::Run(entry) => self.run_entry(entry),
        }
    }

    fn init(&self) -> i32 {
        eprintln!(
            "buildfarm: loaded {} target(s) for init; TUI runner is not implemented yet",
            ConfigFile::load().targets().len()
        );
        2
    }

    fn run_entry(&self, entry: &str) -> i32 {
        eprintln!(
            "buildfarm: loaded {} target(s) for run `{entry}`; TUI runner is not implemented yet",
            ConfigFile::load().targets().len()
        );
        2
    }

    fn usage() -> ! {
        Fatal::raise("Usage: buildfarm init | run <entry>")
    }
}

struct ConfigFile;

impl ConfigFile {
    fn path() -> String {
        env::args()
            .skip(1)
            .collect::<Vec<_>>()
            .windows(2)
            .find(|args| args[0] == "-c" || args[0] == "--config")
            .map(|args| args[1].clone())
            .or_else(|| env::var("BUILDFARM_CONFIG").ok())
            .unwrap_or_else(|| Fatal::raise("BUILDFARM_CONFIG is not set"))
    }

    fn read() -> String {
        fs::read_to_string(Self::path()).unwrap_or_else(|err| Fatal::raise(&format!("buildfarm: failed to read config: {err}")))
    }

    fn load() -> BuildfarmConfig {
        BuildfarmConfig::parse(&Self::read()).unwrap_or_else(|err| Fatal::raise(&format!("buildfarm: {err}")))
    }
}

struct Fatal;

impl Fatal {
    fn raise(msg: &str) -> ! {
        eprintln!("{msg}");
        process::exit(2);
    }
}

fn main() {
    App::run();
}
