#[cfg(test)]
mod tests {
    use crate::http::{RequestSpec, response_body, scalar_to_string};
    use indexmap::IndexMap;
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
}
