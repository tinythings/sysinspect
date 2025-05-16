use indexmap::IndexMap;
use libsysinspect::util;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct FunctionMapper {
    fmap: IndexMap<String, String>,
    data: IndexMap<String, Value>,
}

impl FunctionMapper {
    pub fn new(map: IndexMap<String, String>) -> Self {
        FunctionMapper { fmap: map, data: IndexMap::new() }
    }

    pub(crate) fn set_data(mut self, data: IndexMap<String, Value>) -> Self {
        self.data = data;
        self
    }

    /// Run the function over values.
    /// Consume self, apply each mapping in-place to `self.data`, and return it.
    pub(crate) fn map(&self) -> IndexMap<String, Value> {
        let mut out = self.data.clone();
        for (k, f) in &self.fmap {
            if let Some(val) = self.data.get(k) {
                let new_val = match f.as_str() {
                    "round" => {
                        let num = val.as_f64().unwrap_or(0.0).round();
                        Value::from(num as i64)
                    }
                    "as-int" => {
                        let i = val.as_i64().unwrap_or(0);
                        Value::from(i)
                    }
                    "as-float" => {
                        let fnum = val.as_f64().unwrap_or(0.0);
                        Value::from(fnum)
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
                        Value::Bool(truthy)
                    }
                    "as-str" => Value::from(util::dataconv::to_string(Some(val.clone()))),
                    _ => val.clone(),
                };
                out.insert(k.clone(), new_val);
            }
        }
        out
    }
}
