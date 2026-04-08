use crate::netadd::{
    AddHost, AddOutcome, AddStatus, ArtifactArch, ArtifactFamily, HostOp, MinionCatalogue, NetworkAddWorkflow, PlatformId, actionable_add_error,
    classify_destination_state, is_missing_master_minion, is_waitable_console_miss, managed_roots, marker_matches_managed_root, normalise_host,
    normalise_path, parse, parse_entry, registration_mismatch_id, render_outcomes, render_results, resolve_dest, resolve_remote_path,
    rows_have_traits, startup_sync_ready,
};
use crate::sshprobe::detect::{CpuArch, ExecMode, PlatformFamily, PrivilegeMode, ProbeInfo, ProbePath, ProbePathKind};
use libcommon::SysinspectError;
use libmodpak::mpk::ModPakRepoIndex;
use libsysinspect::{console::ConsoleMinionInfoRow, traits::TraitSource};
use serde_json::json;
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
        exec_mode: ExecMode::Userland,
        privilege: PrivilegeMode::User,
        os_name: "Linux".to_string(),
        release: "6.0".to_string(),
        version: "test".to_string(),
        home: Some("/opt/test-home".to_string()),
        shell: Some("/bin/sh".to_string()),
        tmp: Some("/tmp".to_string()),
        has_sudo: false,
        disk_free_bytes: Some(10),
        disk_free_path: Some("/tmp".to_string()),
        destination: ProbePath {
            kind: ProbePathKind::Home,
            requested: None,
            resolved: Some("/opt/test-home/sysinspect".to_string()),
            writable: true,
        },
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

    assert!(out.contains("OS/ARCH"));
    assert!(!out.contains("STATE"));
    assert!(out.contains("<probe>"));
}

#[test]
fn render_marks_duplicate_add_as_already_added() {
    let out = render_outcomes(
        &[AddOutcome {
            host: AddHost {
                raw: "foo.com".to_string(),
                host: "foo.com".to_string(),
                host_norm: "foo.com".to_string(),
                user: "hans".to_string(),
                path: None,
                path_norm: None,
            },
            display_path: "/opt/test-home/sysinspect".to_string(),
            platform: "Linux/x86_64".to_string(),
            status: AddStatus::AlreadyAdded,
        }],
        HostOp::Add,
    );

    assert!(out.contains("already added"));
}

#[test]
fn render_marks_unmanaged_install_as_not_managed() {
    let out = render_results(
        &[AddOutcome {
            host: AddHost {
                raw: "foo.com".to_string(),
                host: "foo.com".to_string(),
                host_norm: "foo.com".to_string(),
                user: "hans".to_string(),
                path: None,
                path_norm: None,
            },
            display_path: "/opt/test-home/sysinspect".to_string(),
            platform: "Linux/x86_64".to_string(),
            status: AddStatus::NotManaged,
        }],
        HostOp::Add,
    );

    assert!(out.contains("remove first"));
    assert!(out.contains("Onboarding results for 1 host"));
}

#[test]
fn parses_remove_mode() {
    let args = network_args(&["sysinspect", "network", "--remove", "--hn", "foo.com", "-u", "hans"]);
    let wf = NetworkAddWorkflow::from_matches(args.subcommand_matches("network").unwrap()).unwrap();
    let out = wf.render().unwrap();

    assert_eq!(wf.plan().unwrap().items.len(), 1);
    assert!(out.contains("Planned removal for 1 host"));
}

#[test]
fn parses_force_remove_mode() {
    let args = network_args(&["sysinspect", "network", "--remove", "--force", "--hn", "foo.com", "-u", "hans"]);
    let wf = NetworkAddWorkflow::from_matches(args.subcommand_matches("network").unwrap()).unwrap();

    assert!(wf.clone().plan().is_ok());
    assert!(wf.render().unwrap().contains("Planned removal for 1 host"));
}

#[test]
fn parses_force_add_mode() {
    let args = network_args(&["sysinspect", "network", "--add", "--force", "--hn", "foo.com", "-u", "hans"]);
    let wf = NetworkAddWorkflow::from_matches(args.subcommand_matches("network").unwrap()).unwrap();

    assert!(wf.clone().plan().is_ok());
    assert!(wf.render().unwrap().contains("Planned onboarding for 1 host"));
}

#[test]
fn parses_upgrade_mode() {
    let args = network_args(&["sysinspect", "network", "--upgrade", "--hn", "foo.com", "-u", "hans"]);
    let wf = NetworkAddWorkflow::from_matches(args.subcommand_matches("network").unwrap()).unwrap();
    let out = wf.render().unwrap();

    assert_eq!(wf.plan().unwrap().items.len(), 1);
    assert!(out.contains("Planned upgrade for 1 host"));
}

#[test]
fn parses_comma_separated_remove_hostnames() {
    let args = network_args(&["sysinspect", "network", "--remove", "--hostnames", "foo.com,bar.com", "-u", "hans"]);
    let wf = NetworkAddWorkflow::from_matches(args.subcommand_matches("network").unwrap()).unwrap();

    assert_eq!(wf.plan().unwrap().items.len(), 2);
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
    let dst = resolve_dest(Path::new("/opt/test-home"), Some("booya"));

    assert_eq!(dst.input.as_deref(), Some("booya"));
    assert_eq!(dst.path.unwrap(), Path::new("/opt/test-home/booya"));
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
fn extracts_registration_mismatch_id() {
    let msg = "Error registering minion: Error loading protocol data: Registration key mismatch for be806ac5c8134836b316399e21a76a1f: stored old, requested new";

    assert_eq!(registration_mismatch_id(msg).as_deref(), Some("be806ac5c8134836b316399e21a76a1f"));
}

#[test]
fn treats_missing_single_minion_console_result_as_waitable() {
    let err = SysinspectError::MasterGeneralError(
        "Unable to get minion info: Invalid query: Minion info requires one matching minion, but none were found".to_string(),
    );

    assert!(is_waitable_console_miss(&err));
}

#[test]
fn does_not_treat_unrelated_console_errors_as_waitable() {
    let err = SysinspectError::MasterGeneralError("Unable to get minion info: socket exploded".to_string());

    assert!(!is_waitable_console_miss(&err));
}

#[test]
fn resolves_relative_remote_path() {
    let path = resolve_remote_path(Path::new("/opt/test-home"), Some("booya")).unwrap();

    assert_eq!(path, Path::new("/opt/test-home/booya"));
}

#[test]
fn readiness_traits_require_online_and_persisted_identity() {
    let rows = vec![
        ConsoleMinionInfoRow { key: "minion.online".to_string(), value: json!(true), source: TraitSource::Preset },
        ConsoleMinionInfoRow { key: "system.id".to_string(), value: json!("mid-1"), source: TraitSource::Preset },
        ConsoleMinionInfoRow { key: "system.hostname".to_string(), value: json!("humpel"), source: TraitSource::Preset },
    ];

    assert!(rows_have_traits(&rows));
}

#[test]
fn readiness_traits_reject_missing_hostname() {
    let rows = vec![
        ConsoleMinionInfoRow { key: "minion.online".to_string(), value: json!(true), source: TraitSource::Preset },
        ConsoleMinionInfoRow { key: "system.id".to_string(), value: json!("mid-1"), source: TraitSource::Preset },
    ];

    assert!(!rows_have_traits(&rows));
}

#[test]
fn startup_sync_accepts_completed_module_sync_log() {
    assert!(startup_sync_ready("[04/04/2026 16:43:37] - INFO: Syncing modules from 192.168.122.1:4201 done"));
}

#[test]
fn startup_sync_accepts_explicit_disabled_log() {
    assert!(startup_sync_ready("[04/04/2026 16:43:37] - WARN: Module auto-sync on startup is disabled. Call cluster sync to force modules sync."));
}

#[test]
fn startup_sync_rejects_unfinished_startup_log() {
    assert!(!startup_sync_ready("[04/04/2026 16:43:37] - INFO: Checking module integrity"));
}

#[test]
fn managed_roots_accepts_absolute_marker_entries() {
    assert_eq!(managed_roots("root: /opt/sysinspect\ninit: hopstart\n").unwrap(), vec!["/opt/sysinspect".to_string()]);
}

#[test]
fn managed_roots_rejects_relative_marker_entries() {
    assert!(managed_roots("root: sysinspect\ninit: hopstart\n").is_err());
}

#[test]
fn marker_matches_managed_root_accepts_exact_match() {
    assert!(marker_matches_managed_root("/opt/sysinspect", "root: /opt/sysinspect\ninit: hopstart\n"));
}

#[test]
fn marker_matches_managed_root_rejects_missing_invalid_or_mismatched_markers() {
    assert!(!marker_matches_managed_root("/opt/sysinspect", ""));
    assert!(!marker_matches_managed_root("/opt/sysinspect", "root: /srv/sysinspect\ninit: hopstart\n"));
    assert!(!marker_matches_managed_root("/opt/sysinspect", "root: sysinspect\ninit: hopstart\n"));
}

#[test]
fn classify_destination_state_prefers_local_marker_when_destination_exists() {
    assert_eq!(format!("{:?}", classify_destination_state("/opt/sysinspect", true, "root: /opt/sysinspect\ninit: hopstart\n", false)), "Managed");
    assert_eq!(format!("{:?}", classify_destination_state("/opt/sysinspect", true, "", false)), "NotManaged");
}

#[test]
fn classify_destination_state_treats_missing_destination_as_broken() {
    assert_eq!(format!("{:?}", classify_destination_state("/opt/sysinspect", false, "", true)), "Broken");
}

#[test]
fn classify_destination_state_treats_clean_missing_destination_as_absent() {
    assert_eq!(format!("{:?}", classify_destination_state("/opt/sysinspect", false, "", false)), "Absent");
}

#[test]
fn classify_destination_state_does_not_treat_global_machine_id_as_managed_remnant() {
    assert_eq!(format!("{:?}", classify_destination_state("/opt/test-home/sysinspect", false, "", false)), "Absent");
}

#[test]
fn keeps_absolute_remote_path() {
    let path = resolve_remote_path(Path::new("/opt/test-home"), Some("/opt/booya")).unwrap();

    assert_eq!(path, Path::new("/opt/booya"));
}

#[test]
fn actionable_error_maps_duplicate_live_session() {
    assert_eq!(
        actionable_add_error(&SysinspectError::MinionGeneralError("Another minion from this machine is already connected".to_string())),
        "master still sees a stale live session for this minion"
    );
}

#[test]
fn actionable_error_keeps_generic_message() {
    assert_eq!(actionable_add_error(&SysinspectError::MinionGeneralError("plain failure".to_string())), "Error loading minion data: plain failure");
}

#[test]
fn missing_master_minion_errors_are_treated_as_stale_cleanup_state() {
    assert!(is_missing_master_minion(&SysinspectError::MasterGeneralError(
        "Error loading master data: Unable to find minion 6ea18f9b09c8437db99d01fcdc0317a1".to_string()
    )));
    assert!(!is_missing_master_minion(&SysinspectError::MasterGeneralError("socket exploded".to_string())));
}
