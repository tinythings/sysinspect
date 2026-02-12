mod loader_merge_test {
    use libsensors::load;
    use std::{fs, path::Path};
    use tempfile::TempDir;

    fn write(p: &Path, name: &str, content: &str) {
        fs::write(p.join(name), content).unwrap();
    }

    #[test]
    fn test_loader_merges_recursive_cfg_files_and_sorts() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        fs::create_dir_all(root.join("my-crappy-sensors")).unwrap();
        fs::create_dir_all(root.join("my-better-sensors/sub")).unwrap();

        write(
            root.join("my-crappy-sensors").as_path(),
            "a.cfg",
            r#"
sensors:
  interval:
    min: 3
    max: 10
    unit: seconds

  zebra:
    listener: file

  alpha:
    listener: process
"#,
        );

        write(
            root.join("my-better-sensors/sub").as_path(),
            "b.cfg",
            r#"
sensors:
  beta:
    listener: disk
"#,
        );

        write(root, "nope.txt", "sensors: { }");
        write(root, "bad.cfg", "this: is: not: valid: yaml: [");

        let mut spec = load(root).unwrap();

        // interval range from first file
        let ir = spec.interval_range().unwrap();
        assert_eq!(ir.min, 3);
        assert_eq!(ir.max, 10);
        assert_eq!(ir.unit, "seconds");

        // keys sorted alphabetically
        let keys: Vec<String> = spec.items().keys().cloned().collect();
        assert_eq!(keys, vec!["alpha".to_string(), "beta".to_string(), "zebra".to_string()]);
    }

    #[test]
    fn test_loader_first_wins_interval_and_sensor_id() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        fs::create_dir_all(root.join("x")).unwrap();
        fs::create_dir_all(root.join("y")).unwrap();

        write(
            root.join("x").as_path(),
            "1.cfg",
            r#"
sensors:
  interval:
    min: 1
    max: 2
    unit: seconds

  dup:
    listener: file
"#,
        );

        write(
            root.join("y").as_path(),
            "2.cfg",
            r#"
sensors:
  interval:
    min: 99
    max: 100
    unit: hours

  dup:
    listener: process
"#,
        );

        let mut spec = load(root).unwrap();

        // first interval wins
        let ir = spec.interval_range().unwrap();
        assert_eq!(ir.min, 1);
        assert_eq!(ir.max, 2);
        assert_eq!(ir.unit, "seconds");

        // first sensor wins
        let dup = spec.items().get("dup").cloned().unwrap();
        assert_eq!(dup.listener(), "file");
    }

    #[test]
    fn test_loader_ignores_cfg_without_sensors_key() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        write(
            root,
            "x.cfg",
            r#"
not_sensors:
  a:
    listener: file
"#,
        );

        let mut spec = load(root).unwrap();
        assert_eq!(spec.items().len(), 0);
        assert!(spec.interval_range().is_none());
    }
}
