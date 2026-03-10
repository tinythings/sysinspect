mod loader_events_negative_test {
    use libsensors::load;
    use std::{fs, path::Path};
    use tempfile::TempDir;

    fn write(dir: &Path, name: &str, content: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn nested_sensors_cfg_is_ignored_for_sensors_too() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        write(
            root,
            "sensors.cfg",
            r#"
sensors:
  root-one:
    listener: fsnotify
    args: { path: /tmp }
"#,
        );

        write(
            root.join("nested").as_path(),
            "sensors.cfg",
            r#"
sensors:
  should-not-appear:
    listener: procnotify
    args: { process: [bash] }
"#,
        );

        let mut spec = load(root).unwrap();
        let items = spec.items();
        assert!(items.contains_key("root-one"));
        assert!(!items.contains_key("should-not-appear"));
    }

    #[test]
    fn non_mapping_events_are_ignored() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        write(
            root.join("a").as_path(),
            "sensors.cfg",
            "sensors: {}\n",
        );

        write(
            root.join("a").as_path(),
            "a.cfg",
            r#"
sensors:
  x:
    listener: fsnotify
    args: { path: /tmp }
events: [1, 2, 3]
"#,
        );

        let spec = load(root).unwrap();
        assert!(spec.events_config().is_none());
    }
}
