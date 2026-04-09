use pam::Client;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn service_name() -> &'static str {
    "system-auth"
}

#[cfg(target_os = "freebsd")]
pub(crate) fn service_name() -> &'static str {
    "system"
}

/// Authenticate a user using PAM (Pluggable Authentication Modules).
////// # Arguments
/// * `login` - The username to authenticate.
/// * `password` - The password for the user.
////// # Returns
/// * `Ok(())` if authentication is successful.
/// * `Err(pam::PamError)` if authentication fails.
pub fn authenticate(login: &str, password: &str) -> Result<(), pam::PamError> {
    let mut client = Client::with_password(service_name())?;
    client.conversation_mut().set_credentials(login.to_string(), password.to_string());
    log::debug!("Authenticating user: {login:?}");
    client.authenticate()?;

    Ok(())
}

#[cfg(test)]
mod pamauth_ut {
    use super::service_name;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn pam_service_name_matches_linux_policy() {
        assert_eq!(service_name(), "system-auth");
    }

    #[cfg(target_os = "freebsd")]
    #[test]
    fn pam_service_name_matches_freebsd_policy() {
        assert_eq!(service_name(), "system");
    }
}
