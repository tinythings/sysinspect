use colored::Colorize;
use serde::Deserialize;
use serde::de::{Deserializer, Error};
use serde_json::Value::{self, Array, Null, Object};

#[derive(Debug, Deserialize)]
pub struct ModuleDoc {
    pub name: String,

    /// Optional metadata
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,

    /// Required per your validator
    pub description: String,

    /// Optional sections
    #[serde(default, deserialize_with = "unvec")]
    pub arguments: Vec<DocParam>,

    #[serde(default, deserialize_with = "unvec")]
    pub options: Vec<DocOption>,

    #[serde(default, deserialize_with = "unvec")]
    pub examples: Vec<DocExample>,

    #[serde(default)]
    pub returns: Option<DocReturns>,
}

#[derive(Debug, Deserialize)]
pub struct DocParam {
    pub name: String,

    /// optional in your validator
    #[serde(rename = "type", default)]
    pub ty: Option<String>,

    #[serde(default)]
    pub required: bool,

    /// Lua doc uses "description"
    #[serde(default)]
    pub description: String,

    /// Not in your current schema, but keep compatibility if someone adds it
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DocOption {
    pub name: String,

    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct DocExample {
    pub code: String,

    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct DocReturns {
    #[serde(default)]
    pub description: String,

    /// Can be anything JSON-ish
    #[serde(default)]
    pub sample: Option<Value>,
}

fn unvec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    match Value::deserialize(deserializer)? {
        Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(T::deserialize(item).map_err(Error::custom)?);
            }
            Ok(out)
        }
        Object(obj) if obj.is_empty() => Ok(vec![]),
        Null => Ok(vec![]),
        other => Err(Error::custom(format!("expected array ([]) or empty object ({{}}), got {other}"))),
    }
}

pub fn print_mod_manual(doc_val: &Value) {
    let doc: ModuleDoc = match serde_json::from_value(doc_val.clone()) {
        Ok(d) => d,
        Err(err) => {
            eprintln!("Failed to parse module doc: {err}");
            return;
        }
    };

    // ---- Header ----
    println!("{}:\n  {}\n", "Name".bright_yellow(), doc.name.yellow());

    if doc.version.is_some() || doc.author.is_some() {
        println!("{}:", "Meta".bright_yellow());
        if let Some(v) = &doc.version {
            println!("  {} {}", "Version:".bright_white(), v.white());
        }
        if let Some(a) = &doc.author {
            println!("  {} {}", "Author:".bright_white(), a.white());
        }
        println!();
    }

    println!("{}:", "Description".bright_yellow());
    for line in doc.description.trim().lines() {
        println!("  {}", line.yellow());
    }
    println!();

    // ---- Arguments ----
    if !doc.arguments.is_empty() {
        println!("{}", "Keyword arguments:".bright_yellow());
        print_args(&doc.arguments);
    }

    // ---- Options ----
    if !doc.options.is_empty() {
        println!("{}", "Options:".bright_yellow());
        print_options(&doc.options);
    }

    // ---- Examples ----
    if !doc.examples.is_empty() {
        println!("{}", "Usage examples:".bright_yellow());
        for (i, ex) in doc.examples.iter().enumerate() {
            if !ex.description.trim().is_empty() {
                print!("  {} ", ((i + 1).to_string() + ".").yellow());
                for (j, line) in ex.description.trim().lines().enumerate() {
                    println!("{}{}", if j == 0 { "" } else { "  " }, line.yellow());
                }
            } else {
                println!("  Example {}", (i + 1).to_string().yellow());
            }
            for line in ex.code.trim().lines() {
                println!("    {}", line.bright_white());
            }
            println!();
        }
    }

    // ---- Returns ----
    if let Some(ret) = &doc.returns {
        println!("{}", "Returns:".bright_yellow());

        if !ret.description.trim().is_empty() {
            for line in ret.description.trim().lines() {
                println!("  {}", line.yellow());
            }
        }

        if let Some(sample) = &ret.sample {
            match serde_json::to_string_pretty(sample) {
                Ok(s) => {
                    for line in s.trim().lines() {
                        println!("    {}", line.bright_white());
                    }
                }
                Err(_) => println!("    {}", "<unprintable sample>".bright_red()),
            }
        }

        println!();
    }
}

fn print_args(params: &[DocParam]) {
    let name_width = params.iter().map(|p| p.name.len()).max().unwrap_or(0);

    for p in params {
        // Build metadata like: "type: string, required" or "type: string, default: foo"
        let mut meta = String::new();

        if let Some(ty) = &p.ty {
            meta.push_str(&format!("type: {}", ty));
        } else {
            meta.push_str("type: <any>");
        }

        if p.required {
            meta.push_str(", required");
        } else if let Some(def) = &p.default {
            meta.push_str(&format!(", default: {}", def));
        }

        let meta_col = meta.magenta();

        println!("  {:width$} ({})", p.name.bright_green().bold(), meta_col, width = name_width);

        if !p.description.trim().is_empty() {
            for line in p.description.lines() {
                println!("    {}", line.bright_white());
            }
        }

        println!();
    }
}

fn print_options(opts: &[DocOption]) {
    let name_width = opts.iter().map(|o| o.name.len()).max().unwrap_or(0);

    for o in opts {
        println!("  {:width$}", o.name.bright_green().bold(), width = name_width);

        if !o.description.trim().is_empty() {
            for line in o.description.lines() {
                println!("    {}", line.bright_white());
            }
        }

        println!();
    }
}
