mod sensor_run_early_returns_test {
    use libsensors::sensors::fsnotify::FsNotifySensor;
    use libsensors::sensors::mountnotify::MountSensor;
    use libsensors::sensors::sensor::Sensor;
    use libsensors::sspec::SensorConf;
    use serde_json::{from_value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::time::{Duration, timeout};

    fn fs_cfg_missing_path() -> SensorConf {
        from_value(json!({
            "listener": "fsnotify",
            "args": {}
        }))
        .unwrap()
    }

    fn mount_cfg_missing_mountpoints() -> SensorConf {
        from_value(json!({
            "listener": "mountnotify",
            "args": {}
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn fsnotify_run_returns_early_when_path_missing() {
        let s = FsNotifySensor::new("SID".into(), fs_cfg_missing_path());
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();

        timeout(Duration::from_secs(1), s.run(&move |_evt| {
            hits2.fetch_add(1, Ordering::SeqCst);
        }))
        .await
        .unwrap();

        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn mountnotify_run_returns_early_when_mountpoints_missing() {
        let s = MountSensor::new("SID".into(), mount_cfg_missing_mountpoints());
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();

        timeout(Duration::from_secs(1), s.run(&move |_evt| {
            hits2.fetch_add(1, Ordering::SeqCst);
        }))
        .await
        .unwrap();

        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}
