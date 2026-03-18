use super::{ConsoleBootstrap, ConsoleQuery, ConsoleSealed, ensure_console_keypair};
use crate::{
    cfg::mmconf::{CFG_MASTER_KEY_PRI, CFG_MASTER_KEY_PUB},
    rsa::keys::{RsaKey::{Private, Public}, key_to_file, keygen},
};
use rsa::traits::PublicKeyParts;
use sodiumoxide::crypto::secretbox;
use tempfile::tempdir;

#[test]
fn console_bootstrap_roundtrips_session_key() {
    let root = tempdir().unwrap();
    let (master_prk, master_pbk) = keygen(crate::rsa::keys::DEFAULT_KEY_SIZE).unwrap();
    key_to_file(&Private(master_prk.clone()), root.path().to_str().unwrap_or_default(), CFG_MASTER_KEY_PRI).unwrap();
    key_to_file(&Public(master_pbk.clone()), root.path().to_str().unwrap_or_default(), CFG_MASTER_KEY_PUB).unwrap();

    let (client_prk, client_pbk) = ensure_console_keypair(root.path()).unwrap();
    let symkey = secretbox::gen_key();
    let bootstrap = ConsoleBootstrap::new(&client_prk, &client_pbk, &master_pbk, &symkey).unwrap();
    let (opened, _) = bootstrap.session_key(&master_prk).unwrap();

    assert_eq!(opened.0.to_vec(), symkey.0.to_vec());
}

#[test]
fn console_sealed_roundtrips_payload() {
    let payload = ConsoleQuery {
        model: "cmd://cluster/sync".to_string(),
        query: "*".to_string(),
        traits: "".to_string(),
        mid: "".to_string(),
        context: "{\"op\":\"reset\",\"traits\":{}}".to_string(),
    };
    let key = secretbox::gen_key();
    let sealed = ConsoleSealed::seal(&payload, &key).unwrap();
    let opened: ConsoleQuery = sealed.open(&key).unwrap();

    assert_eq!(opened.model, payload.model);
    assert_eq!(opened.query, payload.query);
    assert_eq!(opened.context, payload.context);
}

#[test]
fn ensure_console_keypair_recovers_missing_public_key_from_private_key() {
    let root = tempdir().unwrap();
    let (client_prk, _) = keygen(crate::rsa::keys::DEFAULT_KEY_SIZE).unwrap();
    key_to_file(&Private(client_prk.clone()), root.path().to_str().unwrap_or_default(), crate::cfg::mmconf::CFG_CONSOLE_KEY_PRI).unwrap();

    let (loaded_prk, loaded_pbk) = ensure_console_keypair(root.path()).unwrap();

    assert_eq!(loaded_prk.n().to_bytes_be(), client_prk.n().to_bytes_be());
    assert!(root.path().join(crate::cfg::mmconf::CFG_CONSOLE_KEY_PUB).exists());
    assert_eq!(loaded_pbk.n().to_bytes_be(), client_prk.n().to_bytes_be());
}

#[cfg(unix)]
#[test]
fn ensure_console_keypair_sets_restrictive_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().unwrap();
    let _ = ensure_console_keypair(root.path()).unwrap();

    let dir_mode = std::fs::metadata(root.path()).unwrap().permissions().mode() & 0o777;
    let key_mode = std::fs::metadata(root.path().join(crate::cfg::mmconf::CFG_CONSOLE_KEY_PRI))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;

    assert_eq!(dir_mode, 0o700);
    assert_eq!(key_mode, 0o600);
}
