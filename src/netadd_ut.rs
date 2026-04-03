use crate::netadd::{
    ArtifactArch, ArtifactFamily, MinionCatalogue, NetworkAddWorkflow, PlatformId, normalise_host, normalise_path, parse, parse_entry, resolve_dest,
    resolve_remote_path,
};
use crate::sshprobe::detect::{CpuArch, PlatformFamily, ProbeInfo, ProbePath, ProbePathKind};
use libmodpak::mpk::ModPakRepoIndex;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn network_args(args: &[&str]) -> clap::ArgMatches {
    crate::clidef::cli("test").try_get_matches_from(args).unwrap()
}

fn scratch_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sysinspect-{name}-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn seed_minion_repo() -> PathBuf {
    let root = scratch_dir("modrepo");
    let mut idx = ModPakRepoIndex::new();
    let rel = PathBuf::from("minion/linux/x86_64/sysminion");
    idx.index_minion("linux", "x86_64", rel.clone(), "deadbeef", "0.4.0").unwrap();
    fs::create_dir_all(root.join("minion/linux/x86_64")).unwrap();
    fs::write(root.join(&rel), "sysminion").unwrap();
    fs::write(root.join("mod.index"), idx.to_yaml().unwrap()).unwrap();
    root
}

fn probe_linux_x86_64() -> ProbeInfo {
    ProbeInfo {
        host: "foo.com".to_string(),
        user: "hans".to_string(),
        family: PlatformFamily::Linux,
        arch: CpuArch::X86_64,
        os_name: "Linux".to_string(),
        release: "6.0".to_string(),
        version: "test".to_string(),
        home: Some("/home/hans".to_string()),
        shell: Some("/bin/sh".to_string()),
        tmp: Some("/tmp".to_string()),
        has_sudo: false,
        disk_free_bytes: Some(10),
        disk_free_path: Some("/tmp".to_string()),
        destination: ProbePath { kind: ProbePathKind::System, requested: None, resolved: None, writable: true },
        writable_paths: vec!["/tmp".to_string()],
    }
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
fn platform_id_maps_probe_fields() {
    let id = PlatformId::from_probe(&probe_linux_x86_64()).unwrap();

    assert_eq!(id.family, ArtifactFamily::Linux);
    assert_eq!(id.arch, ArtifactArch::X86_64);
    assert_eq!(id.abi, None);
}

#[test]
fn selects_registered_minion_artefact() {
    let root = seed_minion_repo();
    let cat = MinionCatalogue::open(&root).unwrap();
    let art = cat.select(&PlatformId { family: ArtifactFamily::Linux, arch: ArtifactArch::X86_64, abi: None }).unwrap();

    assert_eq!(art.version, "0.4.0");
    assert_eq!(art.checksum, "deadbeef");
    assert!(art.path.ends_with("minion/linux/x86_64/sysminion"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_missing_registered_minion_artefact() {
    let root = seed_minion_repo();
    let cat = MinionCatalogue::open(&root).unwrap();
    let err = cat.select(&PlatformId { family: ArtifactFamily::FreeBsd, arch: ArtifactArch::X86_64, abi: None }).unwrap_err();

    assert!(err.to_string().contains("Missing sysminion artefact"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_stale_minion_index_entries() {
    let root = seed_minion_repo();
    fs::remove_file(root.join("minion/linux/x86_64/sysminion")).unwrap();
    let cat = MinionCatalogue::open(&root).unwrap();
    let err = cat.select(&PlatformId { family: ArtifactFamily::Linux, arch: ArtifactArch::X86_64, abi: None }).unwrap_err();

    assert!(err.to_string().contains("missing on disk"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workflow_selects_artefact_from_probe() {
    let root = seed_minion_repo();
    let args = network_args(&["sysinspect", "network", "--add", "-n", "foo.com", "-u", "hans"]);
    let wf = NetworkAddWorkflow::from_matches(args.subcommand_matches("network").unwrap()).unwrap();
    let art = wf.select_artifact(&root, &probe_linux_x86_64()).unwrap();

    assert_eq!(art.version, "0.4.0");
    let _ = fs::remove_dir_all(root);
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
