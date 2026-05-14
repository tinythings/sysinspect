#[cfg(test)]
mod tests {
    use crate::connect::check_connectivity;
    use libmodcore::runtime;

    fn make_request(options: &[&str], args: &[(&str, &str)]) -> runtime::ModRequest {
        let mut m = serde_json::Map::new();
        m.insert("options".to_string(), serde_json::Value::Array(options.iter().map(|s| serde_json::Value::String(s.to_string())).collect()));
        let mut am = serde_json::Map::new();
        for (k, v) in args {
            am.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
        m.insert("arguments".to_string(), serde_json::Value::Object(am));
        serde_json::from_value(serde_json::Value::Object(m)).unwrap()
    }

    #[test]
    fn missing_args_returns_error() {
        let rt = make_request(&["connect"], &[]);
        let mut rsp = runtime::new_call_response();
        check_connectivity(&rt, &mut rsp);
        assert_eq!(serde_json::to_value(&rsp).unwrap()["retcode"], 1);
    }

    #[test]
    fn localhost_tcp_is_open() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let rt = make_request(&["connect"], &[("host", "127.0.0.1"), ("port", &addr.port().to_string())]);
        let mut rsp = runtime::new_call_response();
        check_connectivity(&rt, &mut rsp);
        assert_eq!(serde_json::to_value(&rsp).unwrap()["retcode"], 0);
        drop(listener);
    }

    #[test]
    fn closed_port_returns_error() {
        let rt = make_request(&["connect"], &[("host", "127.0.0.1"), ("port", "19999")]);
        let mut rsp = runtime::new_call_response();
        check_connectivity(&rt, &mut rsp);
        assert_eq!(serde_json::to_value(&rsp).unwrap()["retcode"], 1);
    }

    #[test]
    fn invalid_port_is_error() {
        let rt = make_request(&["connect"], &[("host", "127.0.0.1"), ("port", "abc")]);
        let mut rsp = runtime::new_call_response();
        check_connectivity(&rt, &mut rsp);
        assert_eq!(serde_json::to_value(&rsp).unwrap()["retcode"], 1);
    }

    #[test]
    fn telemetry_has_required_fields() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let rt = make_request(&["connect"], &[("host", "127.0.0.1"), ("port", &addr.port().to_string())]);
        let mut rsp = runtime::new_call_response();
        check_connectivity(&rt, &mut rsp);
        let data = &serde_json::to_value(&rsp).unwrap()["data"];
        assert_eq!(data["host"], "127.0.0.1");
        assert!(data["open"].as_bool().unwrap());
        assert!(data["latency_ms"].as_u64().is_some());
        drop(listener);
    }
}
