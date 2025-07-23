use base64::{Engine, engine::general_purpose::STANDARD};
use libsysinspect::{
    SysinspectError,
    rsa::keys::{
        RsaKey::{Private, Public},
        encrypt, key_from_file, key_to_file, keygen,
    },
};
use rpassword::prompt_password;
use serde_json::json;
use std::{
    fs,
    io::{Write, stdin, stdout},
    path::Path,
};
use syswebclient::{
    apis::{authentication_api::authenticate_user, configuration::Configuration, rsa_public_keys_api::master_key},
    models::AuthRequest,
};

static PRIVKEY: &str = "private.key";
static PUBKEY: &str = "public.key";
static MASTER_PUBKEY: &str = "master_public.key";

/// Generate a new RSA key pair and save it to files, if does not already exist.
async fn setup_rsa() -> Result<(), SysinspectError> {
    if !Path::new(PRIVKEY).exists() || !Path::new(PUBKEY).exists() {
        println!("Generating new RSA key pair...");

        let (prk, pbk) = keygen(2048)?;
        key_to_file(&Private(prk), "./", PRIVKEY)?;
        key_to_file(&Public(pbk), "./", PUBKEY)?;

        println!("Keys generated and saved to {} and {}.", PRIVKEY, PUBKEY);
    }

    if !Path::new(MASTER_PUBKEY).exists() {
        // Get the master public key from the server
        let r = master_key(&get_config()).await.map_err(|e| {
            SysinspectError::MasterGeneralError(format!("Failed to retrieve master public key (network): {}", e))
        })?;

        if r.key.is_empty() {
            return Err(SysinspectError::MasterGeneralError("Master public key is empty".to_string()));
        }
        // Save the master public key to a file
        fs::write(MASTER_PUBKEY, r.key.as_bytes()).map_err(|e| SysinspectError::IoErr(e))?;
    }

    Ok(())
}

/// Encrypt data using the public key
fn encrypt_data(data: String, pkey: &str) -> Result<Vec<u8>, SysinspectError> {
    let rsakey = key_from_file(pkey)?.expect("RSA is unfriendly to you at this point :-(");
    let pbk = match rsakey {
        Public(ref k) => k.clone(),
        _ => panic!("Expected a public key"),
    };

    if let Ok(x) = encrypt(pbk, data.as_bytes().to_vec()) {
        return Ok(x);
    }

    return Err(SysinspectError::RSAError("Failed to encrypt data".to_string()));
}

/// Get user credentials from STDIN
fn get_credentials() -> Result<(String, String), SysinspectError> {
    print!("Username: ");
    stdout().flush().unwrap();
    let mut username = String::new();
    stdin().read_line(&mut username).unwrap();

    Ok((username.trim().to_string(), prompt_password("Password: ").unwrap()))
}

/// Get my local public key from a file
fn get_my_pubkey() -> Result<String, SysinspectError> {
    fs::read_to_string(PUBKEY).map_err(|e| SysinspectError::IoErr(e))
}

/// Get *client* configuration for syswebclient
fn get_config() -> Configuration {
    // Configure the syswebclient
    let mut cfg = Configuration::new();
    cfg.base_path = "http://localhost:4202".to_string();
    cfg
}

#[tokio::main]
async fn main() -> Result<(), SysinspectError> {
    // Setup
    setup_rsa().await?; // Or skip if keys already exist

    // Create a request to authenticate a user with an ENCRYPTED username and password.
    // Assuming we have already a master public key to encrypt the credentials.
    let (uid, pwd) = get_credentials()
        .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to read credentials: {}", e)))?;
    let pbk = get_my_pubkey()
        .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to read public key: {}", e)))?;
    let r = authenticate_user(
        &get_config(),
        AuthRequest {
            payload: STANDARD
                .encode(&encrypt_data(json!({"username": uid, "password": pwd}).to_string(), MASTER_PUBKEY)?),
            pubkey: pbk,
        },
    )
    .await
    .map_err(|e| SysinspectError::MasterGeneralError(format!("Authentication error: {}", e)))?;

    println!("Authentication successful, session ID: {:#?}", r);

    Ok(())
}
