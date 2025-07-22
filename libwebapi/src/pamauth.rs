use pam::Client;

/// Authenticate a user using PAM (Pluggable Authentication Modules).
////// # Arguments
/// * `login` - The username to authenticate.
/// * `password` - The password for the user.
////// # Returns
/// * `Ok(())` if authentication is successful.
/// * `Err(pam::PamError)` if authentication fails.
pub fn authenticate(login: &str, password: &str) -> Result<(), pam::PamError> {
    let mut client = Client::with_password("system-auth")?;
    client.conversation_mut().set_credentials(login.to_string(), password.to_string());
    log::debug!("Authenticating user: {login:?}");
    client.authenticate()?;

    Ok(())
}
