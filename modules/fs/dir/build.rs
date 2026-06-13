include!("../../build-help.rs");
fn main() {
    generate_help();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "linux" && target_env == "gnu" {
        println!("cargo:rustc-link-lib=c");
    }
}
