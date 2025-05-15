use indexmap::IndexMap;
use libsysinspect::util;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct MapReducer {
    fmap: IndexMap<String, String>,
    data: IndexMap<String, Value>,
}

impl MapReducer {
    pub fn new(functionmap: IndexMap<String, String>) -> Self {
        MapReducer { fmap: functionmap, data: IndexMap::new() }
    }

    pub(crate) fn set_data(&mut self, data: IndexMap<String, Value>) -> &mut Self {
        self.data = data;
        self
    }

    pub fn data(&self) -> &IndexMap<String, Value> {
        &self.data
    }

    pub fn reduce(&mut self) -> &mut Self {
        self
    }

    /// Run the function over values.
    /// Consume self, apply each mapping in-place to `self.data`, and return it.
    pub(crate) fn map(&mut self) -> &mut Self {
        for (k, f) in &self.fmap {
            if let Some(val) = self.data.get_mut(k) {
                match f.as_str() {
                    "round" => {
                        let num = val.as_f64().unwrap_or(0.0).round();
                        *val = Value::from(num as i64);
                    }
                    "as-int" => {
                        let i = val.as_i64().unwrap_or(0);
                        *val = Value::from(i);
                    }
                    "as-float" => {
                        let fnum = val.as_f64().unwrap_or(0.0);
                        *val = Value::from(fnum);
                    }
                    "as-bool" => {
                        let truthy = match val {
                            Value::Bool(b) => *b,
                            Value::Number(n) => {
                                if let Some(i) = n.as_i64() {
                                    i != 0
                                } else {
                                    n.as_f64().unwrap_or(0.0) != 0.0
                                }
                            }
                            Value::String(s) => !s.is_empty(),
                            Value::Array(a) => !a.is_empty(),
                            Value::Object(o) => !o.is_empty(),
                            Value::Null => false,
                        };
                        *val = Value::Bool(truthy);
                    }
                    "as-str" => {
                        *val = Value::from(util::dataconv::to_string(Some(val.clone())));
                    }
                    _ => {}
                }
            }
        }
        self
    }
}
