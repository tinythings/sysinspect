#[cfg(test)]
mod rsa_test {
    use libsysinspect::rsa::keys::{
        decrypt, encrypt, key_from_file, key_to_file, keygen, sign_data, to_pem, verify_sign,
        RsaKey::{Private, Public},
        DEFAULT_KEY_SIZE,
    };
    use std::fs;

    #[test]
    fn test_keygen() {
        let r = keygen(DEFAULT_KEY_SIZE);
        assert!(r.is_ok(), "Error generating RSA keys");
    }

    #[test]
    fn test_to_pem() {
        let (pr, pb) = keygen(DEFAULT_KEY_SIZE).unwrap();
        let r = to_pem(Some(&pr), Some(&pb));

        assert!(r.is_ok(), "Unable to convert RSA keys to PEM");

        let (prp, pbp) = r.unwrap();

        assert!(prp.is_some(), "Unable to convert private PEM key");
        assert!(pbp.is_some(), "Unable to convert public PEM key");

        let prp = prp.unwrap_or_default();
        let pbp = pbp.unwrap_or_default();

        assert!(prp.contains(" PRIVATE "), "Not a private key");
        assert!(pbp.contains(" PUBLIC "), "Not a public key");
    }

    #[test]
    fn test_sign() {
        let (pr, pb) = keygen(DEFAULT_KEY_SIZE).unwrap();

        let data = "Sysinspect can also configure systems!";
        let sig = sign_data(pr.clone(), data.as_bytes()).unwrap();

        assert!(!sig.is_empty(), "Sig should not be empty");

        let r = verify_sign(&pb, data.as_bytes(), sig);

        assert!(r.is_ok(), "Verification failed to proceed");
        assert!(r.unwrap(), "Verification didn't succeed");
    }

    #[test]
    fn test_cipher() {
        let (pr, pb) = keygen(DEFAULT_KEY_SIZE).unwrap();
        let data = "Sysinspect can also configure systems!";
        let cipher = encrypt(pb.to_owned(), data.as_bytes().to_vec()).unwrap();

        assert!(!cipher.is_empty(), "No cipher found");

        let rdata = String::from_utf8(decrypt(pr.to_owned(), cipher).unwrap()).unwrap_or_default();

        assert!(rdata.eq(data), "Data wasn't properly decryped");
    }

    #[test]
    fn test_to_file() {
        _ = fs::remove_file("priv.key.pem");
        _ = fs::remove_file("pub.key.pem");

        let (pr, pb) = keygen(DEFAULT_KEY_SIZE).unwrap();

        if let Err(err) = key_to_file(&Private(pr), "", "priv.key.pem") {
            assert!(false, "Private key error: {err}");
        };

        if let Err(err) = key_to_file(&Public(pb), "", "pub.key.pem") {
            assert!(false, "Public key error: {err}");
        };

        match key_from_file("priv.key.pem").unwrap().unwrap() {
            Private(_) => assert!(true),
            Public(_) => assert!(false, "Not a public key"),
        }

        match key_from_file("pub.key.pem").unwrap().unwrap() {
            Private(_) => assert!(false, "Not a private"),
            Public(_) => assert!(true),
        }

        _ = fs::remove_file("priv.key.pem");
        _ = fs::remove_file("pub.key.pem");
    }
}
