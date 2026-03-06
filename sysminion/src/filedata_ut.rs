#[cfg(test)]
mod filedata_ut {
    use crate::filedata::SensorsFiledata;
    use libsysinspect::util::iofs::get_file_sha256;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn sensors_sync_is_authoritative_by_path() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        fs::write(root.join("keep.cfg"), "keep").unwrap();
        fs::write(root.join("old.cfg"), "old-local").unwrap();
        fs::create_dir_all(root.join("local/sub")).unwrap();
        fs::write(root.join("local/sub/stale.cfg"), "stale").unwrap();

        let keep_cs = get_file_sha256(root.join("keep.cfg")).unwrap();
        let old_new_tmp = root.join(".tmp-new-old.cfg");
        fs::write(&old_new_tmp, "old-remote").unwrap();
        let old_new_cs = get_file_sha256(old_new_tmp.clone()).unwrap();
        fs::remove_file(old_new_tmp).unwrap();

        let missing_tmp = root.join(".tmp-missing.cfg");
        fs::write(&missing_tmp, "missing-remote").unwrap();
        let missing_cs = get_file_sha256(missing_tmp.clone()).unwrap();
        fs::remove_file(missing_tmp).unwrap();

        let payload = json!({
            "sensors_root": "sensors",
            "files": {
                "/sensors/keep.cfg": keep_cs,
                "/sensors/old.cfg": old_new_cs,
                "/sensors/new/sub/new.cfg": missing_cs
            }
        });

        let sfd = SensorsFiledata::from_payload(payload, root.to_path_buf()).unwrap();

        assert!(sfd.files().contains_key("/sensors/old.cfg"));
        assert!(sfd.files().contains_key("/sensors/new/sub/new.cfg"));
        assert!(!sfd.files().contains_key("/sensors/keep.cfg"));

        assert!(sfd.stale_paths().contains(&"local/sub/stale.cfg".to_string()));
        assert!(!sfd.stale_paths().contains(&"keep.cfg".to_string()));
        assert!(!sfd.stale_paths().contains(&"old.cfg".to_string()));
    }

    #[test]
    fn equal_checksum_different_path_still_downloads() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        fs::write(root.join("foo.cfg"), "same").unwrap();
        let cs = get_file_sha256(root.join("foo.cfg")).unwrap();

        let payload = json!({
            "sensors_root": "sensors",
            "files": {
                "/sensors/bar.cfg": cs
            }
        });

        let sfd = SensorsFiledata::from_payload(payload, root.to_path_buf()).unwrap();

        assert!(sfd.files().contains_key("/sensors/bar.cfg"));
        assert!(sfd.stale_paths().contains(&"foo.cfg".to_string()));
    }

    #[test]
    fn unsafe_remote_paths_are_ignored() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        let payload = json!({
            "sensors_root": "sensors",
            "files": {
                "/sensors/../../etc/passwd": "deadbeef",
                "/sensors/../x.cfg": "cafebabe",
                "/etc/shadow": "aaaa"
            }
        });

        let sfd = SensorsFiledata::from_payload(payload, root.to_path_buf()).unwrap();
        assert!(sfd.files().is_empty());
    }

    #[test]
    fn all_invalid_manifest_entries_do_not_prune_existing_local_files() {
        let td = TempDir::new().unwrap();
        let root = td.path();
        fs::write(root.join("keep.cfg"), "keep").unwrap();

        let payload = json!({
            "sensors_root": "sensors",
            "files": {
                "/sensors/../../evil.cfg": "deadbeef"
            }
        });

        let sfd = SensorsFiledata::from_payload(payload, root.to_path_buf()).unwrap();
        assert!(sfd.stale_paths().is_empty(), "stale pruning should be disabled for all-invalid manifests");
    }
}
