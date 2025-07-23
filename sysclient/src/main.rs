use libsysinspect::SysinspectError;
use rpassword::prompt_password;
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

    let client = SysClient::new(SysClientConfiguration::default());
    match client.authenticate(&uid, &pwd).await {
        Ok(Some(sid)) => {
            println!("Authentication successful, session ID: {sid}");
        }
        Ok(None) => {
            println!("Authentication failed.");
        }
        Err(e) => {
            return Err(SysinspectError::MasterGeneralError(format!("Authentication error: {e}")));
        }
    };

    Ok(())
}
