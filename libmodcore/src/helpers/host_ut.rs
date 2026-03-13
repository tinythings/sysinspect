use crate::helpers::RuntimeHost;
use serde_json::json;

#[test]
fn runtime_host_reads_traits_and_paths() {
    let host = json!({
        "traits": {
            "system.hostname": "minion-a",
            "system.arch": "x86_64"
        },
        "paths": {
            "sharelib": "/srv/share",
            "root": "/srv/root"
        }
    });

    let helper = RuntimeHost::new(&host);

    assert_eq!(helper.trait_value("system.hostname"), Some(json!("minion-a")));
    assert_eq!(helper.trait_value("system.arch"), Some(json!("x86_64")));
    assert!(helper.has_trait("system.hostname"));
    assert!(!helper.has_trait("system.kernel"));
    assert_eq!(helper.path_value("sharelib"), Some(json!("/srv/share")));
    assert_eq!(helper.path_value("root"), Some(json!("/srv/root")));
    assert_eq!(helper.paths(), json!({ "sharelib": "/srv/share", "root": "/srv/root" }));
}

#[test]
fn runtime_host_defaults_missing_sections_to_empty_values() {
    let host = json!({});
    let helper = RuntimeHost::new(&host);

    assert_eq!(helper.trait_value("system.hostname"), None);
    assert!(!helper.has_trait("system.hostname"));
    assert_eq!(helper.path_value("sharelib"), None);
    assert_eq!(helper.paths(), json!({}));
}
