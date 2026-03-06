use clap::{
    Parser,
    builder::{
        Styles,
        styling::{AnsiColor, Style},
    },
};
pub static SHARELIB: &str = "/usr/share/sysinspect/lib/{}"; // Add runtime ID

pub fn monokai_style() -> Styles {
    Styles::styled()
    // section headers: "USAGE", "OPTIONS"
    .header(
        Style::new()
            .fg_color(Some(AnsiColor::Yellow.into()))
            .bold(),
    )
    // the "Usage:" line content
    .usage(
        Style::new()
            .fg_color(Some(AnsiColor::Magenta.into()))
            .bold(),
    )
    // flags and literals: `-m`, `--man`, `--help-on`
    .literal(
        Style::new()
            .fg_color(Some(AnsiColor::Green.into())),
    )
    // metavars / value names: `<PATH>`, `<USERLET>`
    .placeholder(
        Style::new()
            .fg_color(Some(AnsiColor::BrightYellow.into())), // faux-orange
    )
}

#[derive(Parser, Debug)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    color = clap::ColorChoice::Always,
    styles = monokai_style(),
    override_usage = format!("{} [OPTIONS] < <STDIN>", env!("CARGO_PKG_NAME"))
)]
/// CLI definition
pub struct ModuleCli {
    /// Show this runtime module operational manual
    #[arg(short = 'm', long = "man", alias = "manual")]
    man: bool,
}

/// CLI definition implementation
impl ModuleCli {
    /// Is manual requested
    /// # Returns
    /// `true` if manual requested
    /// `false` otherwise
    pub fn is_manual(&self) -> bool {
        self.man
    }
}

#[derive(Parser, Debug)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    color = clap::ColorChoice::Always,
    styles = monokai_style(),
    override_usage = format!("{} [OPTIONS] < <STDIN>", env!("CARGO_PKG_NAME"))
)]
/// CLI definition
pub struct RuntimeModuleCli {
    /// Show this runtime module operational manual
    #[arg(short = 'm', long = "man", alias = "manual")]
    man: bool,

    /// List available runtime modules
    #[arg(short = 'l', long = "list-modules", alias = "list-plugins")]
    modules: bool,

    /// Path where runtime modules are located. Default: /usr/share/sysinspect/lib/{runtime-id}
    #[arg(short = 's', long = "sharelib", value_name = "PATH")]
    sharelib: Option<String>,

    /// Show operational manual for specific module
    #[arg(short = 'i', long = "info", value_name = "MODULE")]
    help_on: Option<String>,
}

/// CLI definition implementation
impl RuntimeModuleCli {
    /// Get sharelib path
    /// # Returns
    /// Sharelib path
    pub fn get_sharelib(&self) -> String {
        self.sharelib.clone().unwrap_or_else(|| SHARELIB.to_string())
    }

    /// Is manual requested
    /// # Returns
    /// `true` if manual requested
    /// `false` otherwise
    pub fn is_manual(&self) -> bool {
        self.man
    }

    /// Is list plugins requested
    /// # Returns
    /// `true` if list plugins requested
    /// `false` otherwise
    pub fn is_list_modules(&self) -> bool {
        self.modules
    }

    /// Get help on specific module
    /// # Returns
    /// Help on module string
    pub fn get_help_on(&self) -> String {
        self.help_on.clone().unwrap_or_default()
    }
}
