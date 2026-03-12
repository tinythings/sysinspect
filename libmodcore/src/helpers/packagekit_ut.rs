use crate::helpers::PackageKitPackage;
use crate::helpers::RuntimePackageKit;

#[test]
fn package_signal_parser_accepts_backend_data_without_installed_prefix() {
    assert_eq!(
        PackageKitPackage::from_signal(1, "cowsay;1.0;noarch;jammy-universe", "Cowsay").expect("package signal should parse"),
        PackageKitPackage {
            info: 1,
            package_id: "cowsay;1.0;noarch;jammy-universe".to_string(),
            name: "cowsay".to_string(),
            version: "1.0".to_string(),
            arch: "noarch".to_string(),
            data: "jammy-universe".to_string(),
            summary: "Cowsay".to_string(),
        }
    );
}

#[test]
fn package_signal_parser_accepts_installed_marker_too() {
    assert_eq!(
        PackageKitPackage::from_signal(1, "cowsay;1.0;noarch;installed:fedora", "Cowsay").expect("package signal should parse"),
        PackageKitPackage {
            info: 1,
            package_id: "cowsay;1.0;noarch;installed:fedora".to_string(),
            name: "cowsay".to_string(),
            version: "1.0".to_string(),
            arch: "noarch".to_string(),
            data: "installed:fedora".to_string(),
            summary: "Cowsay".to_string(),
        }
    );
}

#[test]
fn available_package_id_parser_accepts_non_installed_entries() {
    assert!(RuntimePackageKit::is_available_package_id("cowsay;1.0;noarch;fedora"));
}

#[test]
fn available_package_id_parser_rejects_installed_entries() {
    assert!(!RuntimePackageKit::is_available_package_id("cowsay;1.0;noarch;installed:fedora"));
}

#[test]
fn installed_package_id_parser_accepts_installed_entries() {
    assert!(RuntimePackageKit::is_installed_package_id("cowsay;1.0;noarch;installed:fedora"));
}

#[test]
fn installed_package_id_parser_rejects_available_entries() {
    assert!(!RuntimePackageKit::is_installed_package_id("cowsay;1.0;noarch;fedora"));
}
