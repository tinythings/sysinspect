use super::{SysClient, SysClientConfiguration};
use libcommon::SysinspectError;
use serde_json::json;

#[test]
fn default_configuration_uses_https_localhost() {
    assert_eq!(SysClientConfiguration::default().master_url, "https://localhost:4202");
}

#[tokio::test]
async fn query_requires_authentication_first() {
    let client = SysClient::new(SysClientConfiguration { master_url: "https://localhost:4202".to_string() });
    let err = client.query("cm/file-ops", "*", "", "", json!({})).await.unwrap_err().to_string();

    assert!(err.contains("not authenticated"));
}

#[tokio::test]
async fn models_require_authentication_first() {
    let client = SysClient::new(SysClientConfiguration { master_url: "https://localhost:4202".to_string() });
    let err = client.models().await.unwrap_err().to_string();

    assert!(err.contains("not authenticated"));
}

#[tokio::test]
async fn model_descr_requires_authentication_first() {
    let client = SysClient::new(SysClientConfiguration { master_url: "https://localhost:4202".to_string() });
    let err = client.model_descr("cm").await.unwrap_err().to_string();

    assert!(err.contains("not authenticated"));
}

#[test]
fn query_context_must_be_json_object() {
    let err = match SysClient::context_map(json!(["not", "an", "object"])) {
        Ok(_) => panic!("expected serialization error"),
        Err(SysinspectError::SerializationError(err)) => err,
        Err(other) => panic!("unexpected error: {other}"),
    };

    assert!(err.contains("JSON object"));
}

#[test]
fn query_context_stringifies_non_string_values() {
    let context = SysClient::context_map(json!({
        "n": 42,
        "b": true,
        "s": "text"
    }))
    .unwrap();

    assert_eq!(context.get("n"), Some(&"42".to_string()));
    assert_eq!(context.get("b"), Some(&"true".to_string()));
    assert_eq!(context.get("s"), Some(&"text".to_string()));
}
