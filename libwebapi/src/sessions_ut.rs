use super::SessionStore;

#[test]
fn open_with_sid_rejects_empty_session_id() {
    assert!(SessionStore::new().open_with_sid("dev".to_string(), "   ".to_string()).is_err());
}

#[test]
fn open_with_sid_rejects_sid_owned_by_another_user() {
    let mut store = SessionStore::new();

    assert_eq!(store.open_with_sid("alice".to_string(), "dev-token".to_string()).unwrap(), "dev-token");
    assert!(store.open_with_sid("bob".to_string(), "dev-token".to_string()).is_err());
}

#[test]
fn open_with_sid_reuses_same_sid_for_same_user() {
    let mut store = SessionStore::new();

    assert_eq!(store.open_with_sid("dev".to_string(), "dev-token".to_string()).unwrap(), "dev-token");
    assert_eq!(store.open_with_sid("dev".to_string(), "dev-token".to_string()).unwrap(), "dev-token");
    assert_eq!(store.uid("dev-token").unwrap(), "dev");
}
