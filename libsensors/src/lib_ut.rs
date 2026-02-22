#[cfg(test)]
mod merge_events_test {
    use std::{fs, path::Path};
    use tempfile::TempDir;

    use crate::{load, merged_events_yaml};

    fn write(dir: &Path, name: &str, content: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn root_sensors_cfg_interval_wins() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        // root sensors.cfg defines interval
        write(
            root,
            "sensors.cfg",
            r#"
sensors:
  interval:
    min: 1
    max: 2
    unit: seconds
"#,
        );

        // another chunk tries to override
        write(
            root.join("x").as_path(),
            "x.cfg",
            r#"
sensors:
  interval:
    min: 9
    max: 10
    unit: hours
"#,
        );

        let spec = load(root).unwrap();
        let ir = spec.interval_range().unwrap();
        assert_eq!(ir.min, 1);
        assert_eq!(ir.max, 2);
        assert_eq!(ir.unit, "seconds");
    }

    #[test]
    fn duplicate_event_key_first_wins() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        write(
            root.join("a").as_path(),
            "a.cfg",
            r#"
events:
  dup|x|y|0:
    handlers: [console-logger]
    console-logger:
      prefix: first
"#,
        );

        write(
            root.join("b").as_path(),
            "b.cfg",
            r#"
events:
  dup|x|y|0:
    handlers: [console-logger]
    console-logger:
      prefix: second
"#,
        );

        let ev = merged_events_yaml(root).unwrap();
        let map = ev.as_mapping().unwrap();

        let k = serde_yaml::Value::String("dup|x|y|0".into());
        let v = map.get(&k).unwrap();

        // ensure the first file's value stayed
        let vmap = v.as_mapping().unwrap();
        let handlers = vmap.get(&serde_yaml::Value::String("handlers".into())).unwrap().as_sequence().unwrap();
        assert_eq!(handlers[0].as_str().unwrap(), "console-logger");

        let cl = vmap.get(&serde_yaml::Value::String("console-logger".into())).unwrap().as_mapping().unwrap();
        let prefix = cl.get(&serde_yaml::Value::String("prefix".into())).unwrap().as_str().unwrap();
        assert_eq!(prefix, "first");
    }

    #[test]
    fn merges_events_from_all_cfg_files_and_ignores_nested_sensors_cfg() {
        let td = TempDir::new().unwrap();
        let root = td.path();

        // root sensors.cfg
        write(
            root,
            "sensors.cfg",
            r#"
events:
  root|file|ok@r|0:
    handlers: [console-logger]
"#,
        );

        // normal chunk (events only)
        write(
            root.join("a").as_path(),
            "a.cfg",
            r#"
events:
  a|fsnotify|changed@/tmp/x|0:
    handlers: [console-logger]
"#,
        );

        // another normal chunk
        write(
            root.join("b/sub").as_path(),
            "b.cfg",
            r#"
events:
  b|procnotify|appeared@bash|0:
    handlers: [console-logger]
"#,
        );

        // forbidden nested sensors.cfg must be ignored
        write(
            root.join("b/sub").as_path(),
            "sensors.cfg",
            r#"
events:
  SHOULD|NOT|APPEAR|0:
    handlers: [console-logger]
"#,
        );

        let ev = merged_events_yaml(root).unwrap();
        let map = ev.as_mapping().unwrap();

        assert!(map.contains_key(&serde_yaml::Value::String("root|file|ok@r|0".into())));
        assert!(map.contains_key(&serde_yaml::Value::String("a|fsnotify|changed@/tmp/x|0".into())));
        assert!(map.contains_key(&serde_yaml::Value::String("b|procnotify|appeared@bash|0".into())));
        assert!(!map.contains_key(&serde_yaml::Value::String("SHOULD|NOT|APPEAR|0".into())));
    }
}
