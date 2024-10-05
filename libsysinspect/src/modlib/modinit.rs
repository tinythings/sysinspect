use std::env::args;

use colored::Colorize;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use textwrap::{fill, Options};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModExample {
    description: String,
    code: String,
}

impl ModExample {
    /// Format example
    fn format(&self) -> String {
        format!(
            "  - {}:\n{}\n",
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
            fill(&self.description, Options::new(80).initial_indent("    ").subsequent_indent("    "))
        )
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
    fn get_default(&self) -> String {
        if let Some(def) = &self.default {
            // XXX: Crude stupid hack, which is not so bad after all. :-)
            return Regex::new(r#"\w+\((.*)\)"#).unwrap().replace(&format!("{:?}", def), "$1").to_string().replace('"', "");
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
            fill(&self.description, Options::new(80).initial_indent("    ").subsequent_indent("    "))
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModInterface {
    name: String,
    version: String,
    author: String,
    description: String,
    options: Vec<ModOption>,
    arguments: Vec<ModArgument>,
    examples: Vec<ModExample>,
}

impl ModInterface {
    /// Probably print help. Arguments must contain `--help` or `-h` in the commandline.
    pub fn print_help(&self) {
        let args = args().collect::<Vec<String>>();
        if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
            println!("{}", self.help());
        }
    }

    /// Format help string, ready to print.
    pub fn help(&self) -> String {
        fn args(cls: &ModInterface) -> String {
            let mut out: Vec<String> = Vec::default();
            if !cls.options.is_empty() {
                out.push(format!(
                    "{}\n\n{}",
                    "Options:".bright_yellow(),
                    cls.options.iter().map(|o| o.format()).collect::<Vec<String>>().join("\n")
                ));
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

        let dsc_title = "Description:".bright_yellow();
        let ex_title = "Usage examples:".bright_yellow();
        let ex_code = self.examples.iter().map(|e| e.format()).collect::<Vec<String>>().join("\n");

        format!(
            "{}, {} (Author: {})

{dsc_title}

  {}

{}

{ex_title}

{}",
            self.name.bold(),
            self.version.green().bold(),
            self.author,
            fill(&self.description, Options::new(80).subsequent_indent("  ")).yellow(),
            args(self),
            ex_code
        )
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
