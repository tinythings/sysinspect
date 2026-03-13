use super::{ConsoleBootstrap, ConsoleQuery, ConsoleSealed, ensure_console_keypair};
use crate::{
    cfg::mmconf::{CFG_MASTER_KEY_PRI, CFG_MASTER_KEY_PUB},
    rsa::keys::{RsaKey::{Private, Public}, key_to_file, keygen},
};
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
