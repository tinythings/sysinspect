#[cfg(test)]
mod tests {
    use crate::http::{RequestSpec, response_body, scalar_to_string};
    use indexmap::IndexMap;
    use libmodcore::runtime::ModRequest;
    use serde_json::json;

    /// Scalar values can be converted to strings.
    #[test]
    fn scalar_to_string_accepts_scalars_only() {
        assert_eq!(scalar_to_string(&json!("abc")).as_deref(), Some("abc"));
        assert_eq!(scalar_to_string(&json!(42)).as_deref(), Some("42"));
        assert_eq!(scalar_to_string(&json!(true)).as_deref(), Some("true"));
        assert!(scalar_to_string(&json!({"k":"v"})).is_none());
    }

    /// Query values must stay scalar.
    #[test]
    fn request_spec_query_rejects_nested_values() {
        let spec = RequestSpec::with_query(IndexMap::from([
            ("ok".to_string(), json!("yes")),
            ("bad".to_string(), json!({"nested": true})),
        ]));
        assert!(spec.query().is_err());
    }

    /// JSON text should be parsed into both text and JSON.
    #[test]
    fn response_body_parses_json_text() {
        let body = response_body(br#"{"hello":"world"}"#);
        assert_eq!(body["text"], "{\"hello\":\"world\"}");
        assert_eq!(body["json"]["hello"], "world");
    }

    /// Structured arguments should fail fast on invalid JSON shape.
    #[test]
    fn request_spec_rejects_invalid_structured_arguments() {
        let rt: ModRequest = serde_json::from_value(json!({
            "args": {
                "method": "GET",
                "url": "https://example.com",
                "headers": ["not-an-object"]
            }
        }))
        .unwrap();

        let err = RequestSpec::from_request(&rt).unwrap_err();
        assert!(err.contains("Invalid 'headers' argument"));
    }

    /// Structured config should fail fast on invalid config shape.
    #[test]
    fn request_spec_rejects_invalid_auth_configuration() {
        let rt: ModRequest = serde_json::from_value(json!({
            "args": {
                "method": "GET",
                "url": "https://example.com",
                "auth": ["not-an-object"]
            }
        }))
        .unwrap();

        let err = RequestSpec::from_request(&rt).unwrap_err();
        assert!(err.contains("Invalid 'auth' configuration"));
    }
}
