use colored::Colorize;
use indexmap::IndexMap;
use libsysinspect::util::dataconv;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_yaml::Value;
use std::env::args;
use textwrap::{Options, fill};

static H_WIDTH: usize = 80;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModExample {
    description: String,
    code: String,
}

impl ModExample {
    /// Format example
    fn format(&self) -> String {
        format!(
            "  {}:\n{}\n",
            self.description.yellow(),
            self.code.trim().split("\n").map(|x| format!("      {x}")).collect::<Vec<String>>().join("\n").white()
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModOption {
    name: String,
    description: String,
}

impl ModOption {
    /// Format an option
    fn format(&self) -> String {
        format!(
            "  {}\n{}\n",
            self.name.bright_magenta().bold(),
            fill(&self.description, Options::new(H_WIDTH).initial_indent("    ").subsequent_indent("    "))
        )
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModArgument {
    name: String,
    description: String,

    #[serde(rename = "type")]
    argtype: String,
    required: bool,
    default: Option<Value>,
}

impl ModArgument {
    pub fn get_default(&self) -> String {
        if let Some(def) = &self.default {
            // XXX: Crude stupid hack, which is not so bad after all. :-)
            return Regex::new(r#"\w+\((.*)\)"#).unwrap().replace(&format!("{def:?}"), "$1").to_string().replace('"', "");
        }
        "".to_string()
    }
    /// Format an argument
    fn format(&self) -> String {
        let req = if self.required { ", required".to_string().bold() } else { "".to_string().normal() };
        let def = if self.default.is_some() { format!(", default: {:?}", self.get_default()) } else { "".to_string() };
        format!(
            "  {} {}\n{}\n",
            self.name.bright_magenta().bold(),
            format!("(type: {}{}{})", self.argtype, req, def).bright_magenta(),
            fill(&self.description, Options::new(H_WIDTH).initial_indent("    ").subsequent_indent("    "))
        )
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn required(&self) -> bool {
        self.required
    }

    pub fn argtype(&self) -> &str {
        &self.argtype
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModInterface {
    name: String,
    version: String,
    author: String,
    description: String,
    options: Vec<ModOption>,
    arguments: Vec<ModArgument>,
    examples: Vec<ModExample>,

    // Map of flags/args to output data structure
    returns: IndexMap<String, Value>,
}

impl ModInterface {
    /// Probably print help. Arguments must contain `--help` or `-h` in the commandline.
    pub fn print_help(&self) -> bool {
        let args = args().collect::<Vec<String>>();
        if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
            println!("{}", self.help());
            return true;
        }

        false
    }

    fn fmt_returns(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        for (arg, data) in &self.returns {
            let mut stct: IndexMap<String, serde_json::Value> = IndexMap::new();
            let mut descr = String::new();

            if let Value::Mapping(out_data) = data {
                for (k, v) in out_data {
                    let k = dataconv::as_str(Some(k).cloned());
                    if k.eq(":description") {
                        descr.push_str(dataconv::as_str(Some(v).cloned()).trim());
                    } else {
                        stct.insert(k, json!(v));
                    }
                }
            }

            let f_opts = Options::new(H_WIDTH).initial_indent("  ").subsequent_indent("  ");
            if arg.eq("$") {
                out.push(fill(&format!("{} {}", descr, "If no options or arguments specified:"), &f_opts).yellow().to_string());
            } else {
                out.push(fill(&format!("{} If {} specified:", descr, arg.bright_magenta().bold()), &f_opts).yellow().to_string());
            }

            out.push(fill(
                &serde_json::to_string_pretty(&json!(stct)).unwrap_or_default(),
                f_opts.initial_indent("      ").subsequent_indent("      "),
            ));
            out.push("".to_string());
        }

        out.join("\n")
    }

    /// Format help string, ready to print.
    pub fn help(&self) -> String {
        fn args(cls: &ModInterface) -> String {
            let mut out: Vec<String> = Vec::default();
            if !cls.options.is_empty() {
                out.push(format!("{}\n\n{}", "Options:".bright_yellow(), cls.options.iter().map(|o| o.format()).collect::<Vec<String>>().join("\n")));
            }

            if !cls.arguments.is_empty() {
                out.push(format!(
                    "{}\n\n{}",
                    "Keyword arguments:".bright_yellow(),
                    cls.arguments.iter().map(|o| o.format()).collect::<Vec<String>>().join("\n")
                ));
            }

            out.join("\n")
        }

        fn returns(cls: &ModInterface) -> String {
            let ret_title = "Returned data structure:".bright_yellow();
            format!("\n\n{ret_title}\n\n{}", cls.fmt_returns())
        }

        let dsc_title = "Description:".bright_yellow();
        let ex_title = "Usage examples:".bright_yellow();
        let ex_code = self.examples.iter().map(|e| e.format()).collect::<Vec<String>>().join("\n");

        format!(
            "{}, {} (Author: {})

{dsc_title}

  {}

{}

{ex_title}

{}{}",
            self.name.bold(),
            self.version.green().bold(),
            self.author,
            fill(&self.description, Options::new(H_WIDTH).subsequent_indent("  ")).yellow(),
            args(self),
            ex_code,
            returns(self),
        )
    }

    /// Get the name of the module
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get version
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Get author
    pub fn author(&self) -> &str {
        &self.author
    }

    /// Get description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get options to the module
    pub fn options(&self) -> &[ModOption] {
        &self.options
    }

    /// Get arguments of the module
    pub fn arguments(&self) -> &[ModArgument] {
        &self.arguments
    }
}

/// Include `mod_doc.yaml` from the project and embed it.
#[macro_export]
macro_rules! init_mod_doc {
    ($type:ty) => {{
        const MOD_DOC: &str = include_str!("mod_doc.yaml");
        serde_yaml::from_str::<$type>(MOD_DOC).expect("Wrong schema for mod_doc.yaml")
    }};
}
