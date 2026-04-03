use crate::netadd::{NetworkAddWorkflow, normalise_host, normalise_path, parse, parse_entry, resolve_dest, resolve_remote_path};
use std::{fs, path::Path};

fn network_args(args: &[&str]) -> clap::ArgMatches {
    crate::clidef::cli("test").try_get_matches_from(args).unwrap()
}

#[test]
fn parses_inline_names_with_defaults() {
    let args = network_args(&["sysinspect", "network", "--add", "--hostnames", "foo.com,bar.com", "--user", "hans"]);
    let plan = parse(args.subcommand_matches("network").unwrap()).unwrap();

    assert_eq!(plan.items.len(), 2);
    assert_eq!(plan.items[0].user, "hans");
    assert_eq!(plan.items[0].host, "bar.com");
    assert_eq!(plan.items[1].user, "hans");
    assert_eq!(plan.items[1].host, "foo.com");
}

#[test]
fn inline_user_overrides_default_user() {
    let args = network_args(&["sysinspect", "network", "--add", "-n", "root@foo.com", "-u", "hans"]);
    let plan = parse(args.subcommand_matches("network").unwrap()).unwrap();

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].host, "foo.com");
    assert_eq!(plan.items[0].user, "root");
}

#[test]
fn rejects_missing_input_source() {
    let args = network_args(&["sysinspect", "network", "--add"]);
    let err = parse(args.subcommand_matches("network").unwrap()).unwrap_err();

    assert!(err.to_string().contains("--hostnames") || err.to_string().contains("--list"));
}

#[test]
fn rejects_duplicates_after_normalisation() {
    let args = network_args(&["sysinspect", "network", "--add", "-n", "foo.com,root@foo.com,foo.com", "-u", "root"]);
    let err = parse(args.subcommand_matches("network").unwrap()).unwrap_err();

    assert!(err.to_string().contains("duplicate host entry"));
}

#[test]
fn renders_planned_outcomes() {
    let args = network_args(&["sysinspect", "network", "--add", "--hn", "foo.com", "-u", "hans"]);
    let out = NetworkAddWorkflow::from_matches(args.subcommand_matches("network").unwrap()).unwrap().render().unwrap();

    assert!(out.contains("STATE"));
    assert!(out.contains("planned"));
    assert!(out.contains("validated"));
}

#[test]
fn parses_list_file_with_comments_and_blanks() {
    let list = std::env::temp_dir().join(format!("sysinspect-netadd-{}.txt", std::process::id()));
    fs::write(&list, "\n# comment\nfoo.com\nroot@bar.com:/opt/booya\n").unwrap();
    let args = network_args(&["sysinspect", "network", "--add", "--list", list.to_str().unwrap(), "-u", "hans"]);
    let plan = parse(args.subcommand_matches("network").unwrap()).unwrap();
    let _ = fs::remove_file(&list);

    assert_eq!(plan.items.len(), 2);
    assert_eq!(plan.items[0].host, "bar.com");
    assert_eq!(plan.items[0].user, "root");
    assert_eq!(plan.items[0].path.as_deref(), Some("/opt/booya"));
    assert_eq!(plan.items[1].host, "foo.com");
}

#[test]
fn preserves_raw_input_and_normalises_key_parts() {
    let args = network_args(&["sysinspect", "network", "--add", "-n", "Hans@Foo.COM.:booya/", "-u", "root"]);
    let plan = parse(args.subcommand_matches("network").unwrap()).unwrap();

    assert_eq!(plan.items[0].raw, "Hans@Foo.COM.:booya/");
    assert_eq!(plan.items[0].host_norm, "foo.com");
    assert_eq!(plan.items[0].path_norm.as_deref(), Some("booya"));
    assert_eq!(plan.items[0].user, "Hans");
}

#[test]
fn rejects_invalid_host_grammar() {
    let err = parse_entry("root@@foo").unwrap_err();

    assert!(err.to_string().contains("Invalid input"));
}

#[test]
fn normalises_host_and_path_for_keys() {
    assert_eq!(normalise_host("Foo.COM."), "foo.com");
    assert_eq!(normalise_path(Some("booya/")).as_deref(), Some("booya"));
    assert_eq!(normalise_path(Some("/")).as_deref(), Some("/"));
}

#[test]
fn resolves_destination_against_home() {
    let dst = resolve_dest(Path::new("/home/hans"), Some("booya"));

    assert_eq!(dst.input.as_deref(), Some("booya"));
    assert_eq!(dst.path.unwrap(), Path::new("/home/hans/booya"));
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
