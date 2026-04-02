mod service_start_test {
    use libsensors::service::SensorService;
    use libsensors::sspec::SensorSpec;
    use std::str::FromStr;

    #[tokio::test]
    async fn start_skips_unknown_listener_and_starts_known_one() {
        let y = r#"
sensors:
  ok:
    listener: sys.proc
  bad:
    listener: listener-does-not-exist
"#;
        let spec = SensorSpec::from_str(y).unwrap();
        let mut svc = SensorService::new(spec);

        let handles = svc.start();
        assert_eq!(handles.len(), 1);

        for h in handles {
            h.await.unwrap();
        }
    }

    #[tokio::test]
    async fn start_without_reactor_does_not_panic() {
        let y = r#"
sensors:
  p:
    listener: sys.proc
"#;
        let spec = SensorSpec::from_str(y).unwrap();
        let mut svc = SensorService::new(spec);

        let handles = svc.start();
        assert_eq!(handles.len(), 1);

        for h in handles {
            h.await.unwrap();
        }
    }

    #[tokio::test]
    async fn spawn_returns_abortable_supervisor_handle() {
        let y = r#"
sensors:
  p:
    listener: sys.proc
"#;
        let spec = SensorSpec::from_str(y).unwrap();
        let svc = SensorService::new(spec);

        let h = svc.spawn();
        tokio::task::yield_now().await;
        h.abort();
        let _ = h.await;
    }
}
