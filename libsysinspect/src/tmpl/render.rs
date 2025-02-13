use crate::SysinspectError;
use colored::Colorize;
use indexmap::IndexMap;
use serde_json::Value;
use std::error::Error;
use tera::{Context, Tera};

/// Renderer for the model templates.
///
/// It is using Tera Templates as an engine,
/// similar to Jinja Templates from Django.
/// The model render is designed to use once
/// per a source.
#[derive(Debug, Default)]
pub struct ModelTplRender {
    name: String,
    src: String,
    ctx: Context,
    tpl: Tera,
    res: Option<String>,
}
impl ModelTplRender {
    pub fn new(name: &str, src: &str) -> Self {
        Self { name: name.to_string(), src: src.to_string(), ..Default::default() }
    }

    /// Normalise namespace. No namespace can be a part of other namespace.
    /// For example if there is "system.os.name", then "system.os" is offending namespace.
    /// All namespaces must be completely unique.
    fn normalise_ns(&self, m: &IndexMap<String, Value>) -> Vec<String> {
        let mut keys = m.keys().map(|s| s.to_string()).collect::<Vec<String>>();
        keys.sort();

        let mut bogus = Vec::new();
        for (i, s) in keys.iter().enumerate() {
            if let Some(next_s) = keys.get(i + 1) {
                if next_s.starts_with(s) {
                    bogus.push(s.clone());
                }
            }
        }
        bogus
    }

    fn namespace_exposer(&self, input: IndexMap<String, Value>) -> Value {
        let bogus = self.normalise_ns(&input);
        let mut root = serde_json::Map::new();

        for (key, value) in input {
            if bogus.contains(&key) {
                log::warn!("Skipping bogus namespace: {}", key.bright_yellow().bold());
                continue;
            }

            let parts: Vec<&str> = key.split('.').collect();
            let mut current = &mut root;

            for (i, part) in parts.iter().enumerate() {
                if i == parts.len() - 1 {
                    current.insert(part.to_string(), value.clone());
                } else {
                    current = current
                        .entry(part.to_string())
                        .or_insert_with(|| Value::Object(serde_json::Map::new()))
                        .as_object_mut()
                        .expect("Duplicated namespace keys. This should not happen!");
                }
            }
        }

        Value::Object(root)
    }

    /// Cleans the source for the further processing, removing
    /// empty lines, comments etc.
    fn flatten_src(&self, dst: &str) -> String {
        let mut out: Vec<String> = Vec::default();
        for l in dst.lines() {
            let cl = l.trim();
            if cl.starts_with("#") || cl.is_empty() {
                continue;
            }
            out.push(l.trim_end().to_string());
        }
        out.join("\n")
    }

    /// Set namespaced values so they can be accessed in the template by a namespace.
    /// For example:
    ///
    /// ```text
    ///     {% if mystuff.somebody.name == "Toto" %}
    /// ```
    ///
    /// Namespaces are created from key of a IndexMap and the value
    /// is just a JSON. Example:
    ///
    /// ```rust
    ///     let mut data = IndexMap::new();
    ///     data.insert("somebody.name", json!("Toto"));
    /// ```
    ///
    /// Then register this IndexMap as `mystuff`, passed to the `objname` arg.
    pub fn set_ns_values(&mut self, objname: &str, v: IndexMap<String, Value>) {
        self.ctx.insert(objname, &self.namespace_exposer(v));
    }

    /// Set direct value to be accessed in the template directly.
    pub fn set_value(&mut self, objname: &str, v: Value) {
        self.ctx.insert(objname, &v);
    }

    pub fn render(&mut self) -> Result<String, SysinspectError> {
        // Render only once
        if let Some(res) = self.res.clone() {
            return Ok(res);
        }

        self.tpl.add_raw_template(&self.name, &self.src)?;

        let r = match self.tpl.render(&self.name, &self.ctx) {
            Ok(r) => r,
            Err(err) => {
                let mut iem = String::new();
                if let Some(err) = err.source() {
                    iem = err.to_string();
                }
                return Err(SysinspectError::ModelDSLError(format!("{err}: {iem}")));
            }
        };

        self.res = Some(self.flatten_src(&r));
        Ok(self.res.clone().unwrap_or_default())
    }
}
