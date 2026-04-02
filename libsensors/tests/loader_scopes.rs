mod loader_scopes_test {
    use libsensors::load;
    use std::{fs, path::Path};
    use tempfile::TempDir;

    fn write(dir: &Path, name: &str, content: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn sibling_scope_indexes_are_loaded_without_root_index() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        write(
            root.join("foo").as_path(),
            "sensors.cfg",
            r#"
sensors:
  zed:
    listener: sys.filesystem
    args: { path: /tmp/z }
"#,
        );

        write(
            root.join("foo/sub").as_path(),
            "a.cfg",
            r#"
sensors:
  aaa:
    listener: sys.filesystem
    args: { path: /tmp/a }
"#,
        );

        write(
            root.join("bar").as_path(),
            "sensors.cfg",
            r#"
sensors:
  bbb:
    listener: sys.filesystem
    args: { path: /tmp/b }
"#,
        );

        let mut spec = load(root).unwrap();
        let keys: Vec<_> = spec.items().keys().cloned().collect();
        assert_eq!(keys, vec!["aaa".to_string(), "bbb".to_string(), "zed".to_string()]);
    }

    #[test]
    fn nested_sensors_cfg_is_ignored_but_regular_cfg_is_loaded() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        write(
            root.join("foo").as_path(),
            "sensors.cfg",
            r#"
sensors:
  root-sensor:
    listener: sys.filesystem
    args: { path: /tmp/root }
"#,
        );

        // nested sensors.cfg must be ignored
        write(
            root.join("foo/nested").as_path(),
            "sensors.cfg",
            r#"
sensors:
  must-not-load:
    listener: sys.filesystem
    args: { path: /tmp/nope }
"#,
        );

        // normal cfg under nested dir should still be loaded
        write(
            root.join("foo/nested").as_path(),
            "ok.cfg",
            r#"
sensors:
  nested-ok:
    listener: sys.filesystem
    args: { path: /tmp/ok }
"#,
        );

        let mut spec = load(root).unwrap();
        let items = spec.items();

        assert!(items.contains_key("root-sensor"));
        assert!(items.contains_key("nested-ok"));
        assert!(!items.contains_key("must-not-load"));
    }

    #[test]
    fn cfg_outside_any_scope_is_ignored_when_scopes_exist() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        write(
            root.join("foo").as_path(),
            "sensors.cfg",
            r#"
sensors:
  in-scope:
    listener: sys.filesystem
    args: { path: /tmp/in }
"#,
        );

        write(
            root,
            "orphan.cfg",
            r#"
sensors:
  out-of-scope:
    listener: sys.filesystem
    args: { path: /tmp/out }
"#,
        );

        let mut spec = load(root).unwrap();
        let items = spec.items();

        assert!(items.contains_key("in-scope"));
        assert!(!items.contains_key("out-of-scope"));
    }
}
