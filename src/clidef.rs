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
        .override_usage(format!("{APPNAME} [OPTIONS] [FILTERS]"))

        // Module management
        .subcommand(Command::new("module").about("Add, remove, or list modules").styles(styles.clone()).disable_help_flag(true)
            .arg(Arg::new("add").short('A').long("add").action(ArgAction::SetTrue).help("Add a module to the repository").conflicts_with_all(["remove", "list"]))
            .arg(Arg::new("remove").short('R').long("remove").action(ArgAction::SetTrue).help("Remove a module from the repository").conflicts_with_all(["add", "list"]))
            .arg(Arg::new("list").short('L').long("list").action(ArgAction::SetTrue).help("List all modules in the repository, add --lib to list libraries").conflicts_with_all(["add", "remove"]))
            .arg(Arg::new("match").short('m').long("match").help("Match modules or libraries by an expression"))
            .arg(Arg::new("lib").short('l').long("lib").action(ArgAction::SetTrue).help("Specify that the module is a library (usually Python scripts)"))
            .arg(Arg::new("platform").short('t').long("platform").action(ArgAction::SetTrue).help("Operate on sysminion platform builds instead of modules or libraries. With --remove, use --name=os/arch such as linux/x86_64").conflicts_with("lib"))
            .arg(Arg::new("info").short('i').long("info").action(ArgAction::SetTrue).help("Display information about the module specified by --name"))
            .arg(Arg::new("name").short('n').long("name").help("Specify the module name"))
            .arg(Arg::new("path").short('p').long("path").required_unless_present_any(["help", "list", "remove", "info", "platform"]).help("Specify the path to the module (or library)"))
            .arg(Arg::new("descr").short('d').long("descr").help("Provide a description of the module"))
            .arg(Arg::new("arch").short('a').long("arch").help("Specify the module architecture (x86, x64, arm, arm64, noarch)").default_value("noarch"))
            .arg(Arg::new("help").short('h').long("help").action(ArgAction::SetTrue).help("Display help for this command"))
        )
        .subcommand(Command::new("traits").about("Sync or update minion traits").styles(styles.clone()).disable_help_flag(true)
            .arg(Arg::new("set").long("set").help("Set traits as comma-separated key:value pairs").conflicts_with_all(["unset", "reset"]))
            .arg(Arg::new("unset").long("unset").help("Unset traits as comma-separated keys").conflicts_with_all(["set", "reset"]))
            .arg(Arg::new("reset").long("reset").action(ArgAction::SetTrue).help("Reset all master-managed traits on targeted minions").conflicts_with_all(["set", "unset"]))
            .arg(Arg::new("id").long("id").help("Target a specific minion by its system id").conflicts_with_all(["query", "query-pos"]))
            .arg(Arg::new("query").long("query").help("Target minions by hostname glob or query").conflicts_with("query-pos"))
            .arg(Arg::new("select-traits").long("traits").help("Target minions by traits query"))
            .arg(Arg::new("query-pos").help("Target minions by hostname glob or query").required(false).index(1))
            .arg(Arg::new("help").short('h').long("help").action(ArgAction::SetTrue).help("Display help for this command"))
        )
        .subcommand(Command::new("profile").about("Manage deployment profiles").styles(styles.clone()).disable_help_flag(true)
            .arg(Arg::new("new").long("new").action(ArgAction::SetTrue).help("Create a deployment profile").conflicts_with_all(["delete", "list", "show", "add", "remove", "tag", "untag"]))
            .arg(Arg::new("delete").long("delete").action(ArgAction::SetTrue).help("Delete a deployment profile").conflicts_with_all(["new", "list", "show", "add", "remove", "tag", "untag"]))
            .arg(Arg::new("list").long("list").action(ArgAction::SetTrue).help("List deployment profiles or their assigned selectors").conflicts_with_all(["new", "delete", "show", "add", "remove", "tag", "untag"]))
            .arg(Arg::new("show").long("show").action(ArgAction::SetTrue).help("Show the expanded artefact content of one deployment profile").conflicts_with_all(["new", "delete", "list", "add", "remove", "tag", "untag"]))
            .arg(Arg::new("add").short('A').long("add").action(ArgAction::SetTrue).help("Add selectors to a deployment profile").conflicts_with_all(["new", "delete", "list", "show", "remove", "tag", "untag"]))
            .arg(Arg::new("remove").short('R').long("remove").action(ArgAction::SetTrue).help("Remove selectors from a deployment profile").conflicts_with_all(["new", "delete", "list", "show", "add", "tag", "untag"]))
            .arg(Arg::new("tag").long("tag").help("Assign one or more profiles to targeted minions").conflicts_with_all(["new", "delete", "list", "show", "add", "remove", "untag"]))
            .arg(Arg::new("untag").long("untag").help("Unassign one or more profiles from targeted minions").conflicts_with_all(["new", "delete", "list", "show", "add", "remove", "tag"]))
            .arg(Arg::new("name").short('n').long("name").help("Profile name or profile glob pattern"))
            .arg(Arg::new("match").short('m').long("match").help("Comma-separated module or library selectors"))
            .arg(Arg::new("lib").short('l').long("lib").action(ArgAction::SetTrue).help("Operate on library selectors instead of module selectors"))
            .arg(Arg::new("id").long("id").help("Target a specific minion by its system id").conflicts_with_all(["query", "query-pos"]))
            .arg(Arg::new("query").long("query").help("Target minions by hostname glob or query").conflicts_with("query-pos"))
            .arg(Arg::new("select-traits").long("traits").help("Target minions by traits query"))
            .arg(Arg::new("query-pos").help("Target minions by hostname glob or query").required(false).index(1))
            .arg(Arg::new("help").short('h').long("help").action(ArgAction::SetTrue).help("Display help for this command"))
        )
        .subcommand(Command::new("network").about("Manage cluster transport state and rotation").styles(styles.clone()).disable_help_flag(true)
            .arg(Arg::new("add").short('A').long("add").action(ArgAction::SetTrue).help("Plan onboarding for one or more hosts").conflicts_with_all(["remove", "upgrade", "rotate", "status", "online", "info"]))
            .arg(Arg::new("remove").short('R').long("remove").action(ArgAction::SetTrue).help("Remove one or more managed hosts").conflicts_with_all(["add", "upgrade", "rotate", "status", "online", "info"]))
            .arg(Arg::new("upgrade").long("upgrade").action(ArgAction::SetTrue).help("Upgrade one or more locally managed hosts").conflicts_with_all(["add", "remove", "rotate", "status", "online", "info"]))
            .arg(Arg::new("force").short('f').long("force").action(ArgAction::SetTrue).help("For --remove, forget the minion on the master even when the host cannot be reached. For --add, remove any existing managed install first, then add again.").conflicts_with_all(["upgrade", "rotate", "status", "online", "info"]))
            .arg(Arg::new("rotate").short('r').long("rotate").action(ArgAction::SetTrue).help("Rotate managed transport keys for the selected minions").conflicts_with("status"))
            .arg(Arg::new("status").short('s').long("status").action(ArgAction::SetTrue).help("Show managed transport key status for the selected minions").conflicts_with("rotate"))
            .arg(Arg::new("online").short('o').long("online").action(ArgAction::SetTrue).help("Show online minions for the current selection").conflicts_with_all(["add", "rotate", "status", "info"]))
            .arg(Arg::new("info").long("info").action(ArgAction::SetTrue).help("Show detailed minion registry information for exactly one minion selected by name or --id").conflicts_with_all(["add", "rotate", "status", "online"]))
            .arg(Arg::new("hostnames").short('n').long("hostnames").visible_alias("hn").alias("names").help("Comma-separated host onboarding entries").conflicts_with("list"))
            .arg(Arg::new("list").long("list").help("Path to a file containing one onboarding entry per line").conflicts_with("hostnames"))
            .arg(Arg::new("user").short('u').long("user").help("Default SSH login user for host onboarding"))
            .arg(Arg::new("all").short('a').long("all").action(ArgAction::SetTrue).help("Show all transport states (default)").conflicts_with_all(["pending", "idle"]))
            .arg(Arg::new("pending").short('p').long("pending").action(ArgAction::SetTrue).help("Show only minions with non-idle rotation state").conflicts_with_all(["all", "idle"]))
            .arg(Arg::new("idle").short('i').long("idle").action(ArgAction::SetTrue).help("Show only minions with idle rotation state").conflicts_with_all(["all", "pending"]))
            .arg(Arg::new("id").long("id").help("Target a specific minion by its system id").conflicts_with_all(["query", "query-pos", "add", "upgrade", "hostnames", "list", "user"]))
            .arg(Arg::new("query").long("query").help("Target minions by hostname glob or query").conflicts_with_all(["query-pos", "add", "upgrade", "hostnames", "list", "user"]))
            .arg(Arg::new("select-traits").long("traits").help("Target minions by traits query").conflicts_with_all(["add", "upgrade", "hostnames", "list", "user"]))
            .arg(Arg::new("rotate-overlap").long("rotate-overlap").help("Rotation grace overlap in seconds before retiring old keys").default_value("900"))
            .arg(Arg::new("rotate-reason").long("rotate-reason").help("Operator-visible reason attached to rotation intents").default_value("manual"))
            .arg(Arg::new("query-pos").help("Target minions by hostname glob or query").required(false).index(1).default_value("*"))
            .arg(Arg::new("help").short('h').long("help").action(ArgAction::SetTrue).help("Display help for this command"))
        )

        // Sysinspect
        .next_help_heading("Main")
        .arg(
            Arg::new("path")
                .help("Specify the model path that needs to be requested")
                .required(false)
                .index(1)
        )
        .arg(
            Arg::new("query")
                .help("Specify the minions to query")
                .required(false)
                .index(2)
        )
        .arg(
            Arg::new("traits")
                .short('t')
                .long("traits")
                .help("Specify traits to select remote systems")
        )
        .arg(
            Arg::new("context")
                .short('x')
                .long("context")
                .help(format!("Provide context data as comma-separated key-value pairs to minions when evaluating and running the model.\n{}",
                              "Example: --context='myvar:123,myothervar:\"John Smith\"'".yellow()))
        )

        // Local
        .next_help_heading("Local")

        // Config
        .arg(
            Arg::new("model")
            .short('m')
            .long("model")
            .help("Specify the system description model")
        )
        .arg(
            Arg::new("labels")
            .short('l')
            .long("labels")
            .help("Select only specific labels from the checkbook (comma-separated values)")
            .conflicts_with_all(["entities"])
        )
        .arg(
            Arg::new("entities")
            .short('e')
            .long("entities")
            .help("Select only specific entities from the inventory (comma-separated values)")
            .conflicts_with_all(["labels"])
        )
        .arg(
            Arg::new("state")
                .short('s')
                .long("state")
                .help("Specify a state to process. If none is specified, the default state ($) is used.")
        )

        // Cluster
        .next_help_heading("Cluster")
        .arg(
            Arg::new("ui")
            .short('u')
            .long("ui")
            .action(ArgAction::SetTrue)
            .help("Run the terminal user interface (TUI) application to review the results")
        )
        .arg(
            Arg::new("unregister")
                .long("unregister")
                .help("Unregister a minion by its System ID, hostname, or IP. A new registration will be required.")
        )
        .arg(
            Arg::new("sync")
                .long("sync")
                .action(ArgAction::SetTrue)
                .help(format!("Sync the {} for all artefacts (modules, libraries, traits etc)", "entire cluster".bright_red()))
        )
        .arg(
            Arg::new("shutdown")
                .long("shutdown")
                .action(ArgAction::SetTrue)
                .help(format!("Notify the running master to shut down the {}, be careful! :)", "entire cluster".bright_red()))
        )

        .next_help_heading("Model")
        .arg(
            Arg::new("list-handlers")
                .long("list-handlers")
                .action(ArgAction::SetTrue)
                .help("List available event handler IDs")
        )


        // Other
        .next_help_heading("Other")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Specify an alternative configuration")
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .action(ArgAction::Count)
                .help("Enable debug mode for more verbose output. Increase this flag for greater verbosity."),
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
                .help("Get the current version."),
        )
        .disable_help_flag(true) // Otherwise, it is displayed in the wrong position
        .disable_version_flag(true)
        .disable_colored_help(false)
        .styles(styles)
        .after_help("NOTE: SysInspect is in early development.
      If it does not work as expected, please submit a bug report here:
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

    am.get_one::<String>(id).unwrap_or(&"".to_string()).split(fsep).map(|s| s.to_string()).filter(|s| !s.is_empty()).collect::<Vec<String>>()
}
