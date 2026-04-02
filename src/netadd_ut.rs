use crate::netadd::{parse, resolve_remote_path};
use std::path::Path;

fn network_args(args: &[&str]) -> clap::ArgMatches {
    crate::clidef::cli("test").try_get_matches_from(args).unwrap()
}

#[test]
fn parses_inline_names_with_defaults() {
    let args = network_args(&["sysinspect", "network", "--add", "--names", "foo.com,bar.com", "--user", "hans"]);
    let plan = parse(args.subcommand_matches("network").unwrap()).unwrap();

    assert_eq!(plan.items.len(), 2);
    assert_eq!(plan.items[0].user, "hans");
    assert_eq!(plan.items[0].host, "bar.com");
    assert_eq!(plan.items[1].user, "hans");
    assert_eq!(plan.items[1].host, "foo.com");
}

#[test]
fn inline_user_overrides_default_user() {
    let args = network_args(&["sysinspect", "network", "--add", "--names", "root@foo.com", "--user", "hans"]);
    let plan = parse(args.subcommand_matches("network").unwrap()).unwrap();

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].host, "foo.com");
    assert_eq!(plan.items[0].user, "root");
}

#[test]
fn rejects_missing_input_source() {
    let args = network_args(&["sysinspect", "network", "--add"]);
    let err = parse(args.subcommand_matches("network").unwrap()).unwrap_err();

    assert!(err.to_string().contains("--names") || err.to_string().contains("--list"));
}

#[test]
fn resolves_relative_remote_path() {
    let path = resolve_remote_path(Path::new("/home/hans"), Some("booya")).unwrap();

    assert_eq!(path, Path::new("/home/hans/booya"));
}

#[test]
fn keeps_absolute_remote_path() {
    let path = resolve_remote_path(Path::new("/home/hans"), Some("/opt/booya")).unwrap();

    assert_eq!(path, Path::new("/opt/booya"));
}
