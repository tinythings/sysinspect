//! Example Sysinspect Client
//!
//! This example demonstrates how to use the Sysinspect client to authenticate a user.
//! It prompts the user for their username and password, then attempts to authenticate with the Sysinspect
//! server. If successful, it prints the session ID; otherwise, it indicates that authentication failed.
//!

use libsysinspect::SysinspectError;
use rpassword::prompt_password;
use serde_json::json;
use std::io::{Write, stdin, stdout};
use sysinspect_client_example::{SysClient, SysClientConfiguration};

/// Get user credentials from STDIN
fn get_credentials() -> Result<(String, String), SysinspectError> {
    print!("Username: ");
    stdout().flush().unwrap();
    let mut username = String::new();
    stdin().read_line(&mut username).unwrap();

    Ok((username.trim().to_string(), prompt_password("Password: ").unwrap()))
}

#[tokio::main]
async fn main() -> Result<(), SysinspectError> {
    let (uid, pwd) = get_credentials()
        .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to read credentials: {e}")))?;

    let mut client = SysClient::new(SysClientConfiguration::default());
    match client.authenticate(&uid, &pwd).await {
        Ok(sid) => {
            println!("Authentication successful, session ID: {sid}");
        }
        Err(e) => {
            return Err(SysinspectError::MasterGeneralError(format!("Authentication error: {e}")));
        }
    };

    let r = client.query("cm/file-ops", "*", "", "", json!({"metaid": "12345", "tgt": "it is alive!"})).await?;
    println!("Query result: {}", r.message);

    Ok(())
}
