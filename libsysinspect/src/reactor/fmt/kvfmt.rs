use super::formatter::StringFormatter;
use colored::Colorize;
use prettytable::{
    format::{self},
    Cell, Row, Table,
};
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

pub struct KeyValueFormatter {
    data: Value,
}

impl KeyValueFormatter {
    /// Get visual length of the Unicode cell, taking to the account
    /// terminal escapes
    fn vlen(&self, s: &str) -> usize {
        s.graphemes(true).count()
    }

    fn to_table(&self, tbl: &mut Table, key: &str, v: &Value, offset: usize) {
        let space = "  ".repeat(offset); // Add indentation for nested objects

        match v {
            Value::Object(map) => {
                tbl.add_row(Row::new(vec![Cell::new(&format!("{space}{key}")), Cell::new("")]));
                for (nkey, nval) in map {
                    self.to_table(tbl, &nkey.yellow().to_string(), nval, offset + 1);
                }
            }
            Value::Array(arr) => {
                tbl.add_row(Row::new(vec![Cell::new(&format!("{space}{key}")), Cell::new("")]));
                for elem in arr.iter() {
                    //self.to_table(table, &format!("{}", i + 1), elem, indent + 1);
                    self.to_table(tbl, "", elem, offset + 1);
                }
            }
            Value::String(s) => {
                let cval = s.bright_green().to_string();
                tbl.add_row(Row::new(vec![
                    Cell::new(&format!("{space}{key}")),
                    Cell::new(&format!("{:<width$}", cval, width = self.vlen(&cval))),
                ]));
            }
            Value::Number(n) => {
                let cval = n.to_string().bright_cyan();
                tbl.add_row(Row::new(vec![
                    Cell::new(&format!("{space}{key}")),
                    Cell::new(&format!("{:<width$}", cval, width = self.vlen(&cval))),
                ]));
            }
            Value::Bool(b) => {
                let cval = b.to_string().bright_red();
                tbl.add_row(Row::new(vec![
                    Cell::new(&format!("{space}{key}")),
                    Cell::new(&format!("{:<width$}", cval, width = self.vlen(&cval))),
                ]));
            }
            Value::Null => {
                let cval = "null".yellow();
                tbl.add_row(Row::new(vec![
                    Cell::new(&format!("{space}{key}")),
                    Cell::new(&format!("{:<width$}", cval, width = self.vlen(&cval))),
                ]));
            }
        }
    }

    fn fmt(&self) -> String {
        let mut table = Table::new();

        // Add headers
        table.add_row(Row::new(vec![Cell::new("Key"), Cell::new("Value")]));

        // Start processing the root object
        if let Value::Object(map) = &self.data {
            for (key, value) in map {
                self.to_table(&mut table, &key.bright_yellow().bold().to_string(), value, 0);
            }
        }

        // Print the table
        table.set_format(*format::consts::FORMAT_CLEAN);
        table.to_string()
    }
}

impl StringFormatter for KeyValueFormatter {
    fn new(data: Value) -> Self
    where
        Self: Sized,
    {
        KeyValueFormatter { data }
    }

    /// Format the data
    fn format(&self) -> String {
        self.fmt()
    }
}
