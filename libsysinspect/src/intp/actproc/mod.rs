use serde_json::Value;

pub mod modfinder;
pub mod response;

pub fn get_by_ns(data: Option<Value>, namespace: &str) -> Option<Value> {
    if let Some(ref data) = data {
        let ns: Vec<&str> = namespace.split('.').collect();

        if let Some(v) = get_ns_val(data, &ns) {
            println!("{} = {:?}", namespace, v);
        }
    }

    None
}

fn get_ns_val(data: &Value, ns: &[&str]) -> Option<Value> {
    for n in ns {
        match data {
            Value::Array(a) => {
                for v in a {
                    if let Some(v) = get_ns_val(v, ns) {
                        return Some(v.to_owned());
                    } else {
                        get_ns_val(v, ns);
                    }
                }
            }
            Value::Object(v) => {
                if let Some(v) = v.get(&n.to_string()) {
                    return get_ns_val(v, ns);
                }
            }
            _ => return Some(data.to_owned()),
        }
    }
    None
}
