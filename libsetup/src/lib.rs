use std::env;

pub mod mnsetup;

/// Get SSH user IP.
///
/// This basically reaps-off $SSH_CLIENT variable from the environment,
/// in a hope to find out the IP address of the remote host,
/// which is assumed to be a master host.
pub fn get_ssh_client_ip() -> Option<String> {
    if let Ok(ssh_client) = env::var("SSH_CLIENT") {
        let parts: Vec<&str> = ssh_client.split_whitespace().collect();
        if parts.len() > 0 {
            return Some(parts[0].to_string());
        }
    }

    None
}
