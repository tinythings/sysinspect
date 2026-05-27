use super::osinfo::{os_family, os_release_value};
use std::fs;

#[test]
fn os_family_maps_correctly() {
    assert_eq!(os_family("linux"), "Linux");
    assert_eq!(os_family("android"), "Linux");
    assert_eq!(os_family("freebsd"), "BSD");
    assert_eq!(os_family("netbsd"), "BSD");
    assert_eq!(os_family("openbsd"), "BSD");
    assert_eq!(os_family("dragonfly"), "BSD");
    assert_eq!(os_family("macos"), "Darwin");
    assert_eq!(os_family("ios"), "Darwin");
    assert_eq!(os_family("solaris"), "SunOS");
    assert_eq!(os_family("illumos"), "SunOS");
    assert_eq!(os_family("tatooine"), "tatooine");
}

#[test]
fn os_release_parses_freedesktop_format() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("os-release");
    fs::write(
        &path,
        r#"NAME="Ubuntu"
VERSION="24.04 LTS (Noble Numbat)"
ID=ubuntu
ID_LIKE=debian
PRETTY_NAME="Ubuntu 24.04 LTS"
VERSION_ID="24.04"
"#,
    )
    .unwrap();

    assert_eq!(os_release_value(path.to_str().unwrap(), "ID"), Some("ubuntu".to_string()));
    assert_eq!(os_release_value(path.to_str().unwrap(), "NAME"), Some("Ubuntu".to_string()));
    assert_eq!(os_release_value(path.to_str().unwrap(), "VERSION_ID"), Some("24.04".to_string()));
    assert_eq!(os_release_value(path.to_str().unwrap(), "ID_LIKE"), Some("debian".to_string()));
}

#[test]
fn os_release_handles_empty_and_comments() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("os-release");
    fs::write(
        &path,
        r#"# This is a comment

ID=dragonfly
# Another comment
NAME="DragonFly BSD"
"#,
    )
    .unwrap();

    assert_eq!(os_release_value(path.to_str().unwrap(), "ID"), Some("dragonfly".to_string()));
    assert_eq!(os_release_value(path.to_str().unwrap(), "NAME"), Some("DragonFly BSD".to_string()));
}

#[test]
fn os_release_missing_key_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("os-release");
    fs::write(&path, "ID=endor\n").unwrap();

    assert_eq!(os_release_value(path.to_str().unwrap(), "NAME"), None);
    assert_eq!(os_release_value(path.to_str().unwrap(), "VERSION"), None);
}

#[test]
fn os_release_missing_file_returns_none() {
    assert_eq!(os_release_value("/nonexistent/deathstar/os-release", "ID"), None);
}
