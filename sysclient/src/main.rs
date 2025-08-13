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
use sysinspect_client::{SysClient, SysClientConfiguration};

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
    //let (uid, pwd) = get_credentials()
    //    .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to read credentials: {e}")))?;
    let (uid, pwd) = ("kenpit".to_string(), "kenpit".to_string());
    let mut cfg = SysClientConfiguration::default();
    cfg.master_url = "http://eval220.eso.local:4202".to_string();

    let mut client = SysClient::new(cfg);
    match client.authenticate(&uid, &pwd).await {
        Ok(sid) => {
            println!("Authentication successful, session ID: {sid}");
        }
        Err(e) => {
            return Err(SysinspectError::MasterGeneralError(format!("Authentication error: {e}")));
        }
    };

    let r = client
        .query(
            "kenpit/drain/collect",
            "*",
            "",
            "",
            json!({"metaid": "4fded3d0-43d9-45bc-9bca-d6530ded974b", "data": "fustercluck2", "clusters": "4000"}),
        )
        .await?;
    println!("Query result: {}", r.message);

    Ok(())
}
