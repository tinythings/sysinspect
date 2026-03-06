mod bridge_emitter_test {
    use libsensors::bridge::reactor_emitter;
    use libsysinspect::reactor::evtproc::EventProcessor;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[test]
    fn no_reactor_is_noop() {
        let emit = reactor_emitter("sid-a".to_string(), None);
        emit(json!({"data": {"action": "x"}}));
    }

    #[tokio::test]
    async fn explicit_eid_is_used_for_registering() {
        let reactor = Arc::new(Mutex::new(EventProcessor::new()));
        let emit = reactor_emitter("sid-a".to_string(), Some(reactor.clone()));

        emit(json!({"eid": "custom|eid|x|0", "sensor":"sid-a", "data":{"k":"v"}}));
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let got = reactor.lock().await.receiver().get_by_eid("custom|eid|x|0".to_string());
        assert!(got.is_some());
    }

    #[tokio::test]
    async fn missing_eid_falls_back_to_sensor_id() {
        let reactor = Arc::new(Mutex::new(EventProcessor::new()));
        let emit = reactor_emitter("sid-b".to_string(), Some(reactor.clone()));

        emit(json!({"sensor":"sid-b", "data":{"action":"opened"}}));
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let got = reactor.lock().await.receiver().get_by_eid("sid-b".to_string());
        assert!(got.is_some());
    }
}
