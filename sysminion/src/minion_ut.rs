#[cfg(test)]
mod tests {
    use crate::minion::{_minion_instance, MINION_SID, SysMinion};
    use crate::proto::msg::{CONNECTION_TX, ExitState};
    use crate::rsa::MinionRSAKeyManager;
    use libdpq::DiskPersistentQueue;
    use libcommon::SysinspectError;
    use libsysinspect::{
        cfg::mmconf::{CFG_MASTER_KEY_PUB, MinionConfig},
        rsa::keys::{RsaKey::Public, key_to_file, keygen},
        transport::{
            TransportKeyExchangeModel, TransportKeyStatus, TransportPeerState, TransportProvisioningMode, TransportRotationStatus, TransportStore,
            secure_bootstrap::SecureBootstrapSession,
            secure_channel::{SecureChannel, SecurePeerRole},
        },
    };
    use libsysproto::secure::{SECURE_PROTOCOL_VERSION, SecureFrame};
    use once_cell::sync::Lazy;
    use rsa::RsaPublicKey;
    use std::io::ErrorKind;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;
    use tokio::sync::oneshot;
    use tokio::time::{Duration, timeout};

    #[cfg(target_os = "linux")]
    fn reconnect_drop_wait() -> Duration {
        Duration::from_millis(150)
    }

    #[cfg(not(target_os = "linux"))]
    fn reconnect_drop_wait() -> Duration {
        Duration::from_millis(500)
    }

    #[cfg(target_os = "linux")]
    fn reconnect_boot_wait() -> Duration {
        Duration::from_millis(400)
    }

    #[cfg(not(target_os = "linux"))]
    fn reconnect_boot_wait() -> Duration {
        Duration::from_secs(2)
    }

    #[cfg(target_os = "linux")]
    fn reconnect_accept_timeout() -> Duration {
        Duration::from_secs(10)
    }

    #[cfg(not(target_os = "linux"))]
    fn reconnect_accept_timeout() -> Duration {
        Duration::from_secs(30)
    }

    #[cfg(target_os = "linux")]
    fn reconnect_exit_timeout() -> Duration {
        Duration::from_secs(5)
    }

    #[cfg(not(target_os = "linux"))]
    fn reconnect_exit_timeout() -> Duration {
        Duration::from_secs(15)
    }

    fn secure_state(master_pbk: &RsaPublicKey, minion_pbk: &RsaPublicKey) -> TransportPeerState {
        let mut state = TransportPeerState::new(
            "mid-1".to_string(),
            libsysinspect::rsa::keys::get_fingerprint(master_pbk).unwrap(),
            libsysinspect::rsa::keys::get_fingerprint(minion_pbk).unwrap(),
            SECURE_PROTOCOL_VERSION,
        );
        state.key_exchange = TransportKeyExchangeModel::EphemeralSessionKeys;
        state.provisioning = TransportProvisioningMode::Automatic;
        state.rotation = TransportRotationStatus::Idle;
        state.active_key_id = Some("kid-1".to_string());
        state.last_key_id = Some("kid-1".to_string());
        state
    }

    fn secure_channels(root: &std::path::Path) -> (SecureChannel, SecureChannel) {
        let keyman = MinionRSAKeyManager::new(root.to_path_buf()).unwrap();
        let (master_prk, master_pbk) = libsysinspect::rsa::keys::keygen(2048).unwrap();
        let (_, minion_pbk) = libsysinspect::rsa::keys::from_pem(None, Some(&keyman.get_pubkey_pem())).unwrap();
        let state = secure_state(&master_pbk, &minion_pbk.unwrap());
        let (opening, hello) = SecureBootstrapSession::open(&state, &keyman.private_key().unwrap(), &master_pbk).unwrap();
        let accepted = SecureBootstrapSession::accept(
            &state,
            match &hello {
                SecureFrame::BootstrapHello(hello) => hello,
                _ => panic!("expected bootstrap hello"),
            },
            &master_prk,
            &libsysinspect::rsa::keys::from_pem(None, Some(&keyman.get_pubkey_pem())).unwrap().1.unwrap(),
            Some("sid-1".to_string()),
            Some("kid-1".to_string()),
            None,
        )
        .unwrap();

        (
            SecureChannel::new(SecurePeerRole::Master, &accepted.0).unwrap(),
            SecureChannel::new(
                SecurePeerRole::Minion,
                &opening
                    .verify_ack(
                        &state,
                        match &accepted.1 {
                            SecureFrame::BootstrapAck(ack) => ack,
                            _ => panic!("expected bootstrap ack"),
                        },
                        &master_pbk,
                    )
                    .unwrap(),
            )
            .unwrap(),
        )
    }

    static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn mk_cfg(master: String, _fileserver: String, root: &std::path::Path) -> MinionConfig {
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(master.split(':').next().unwrap_or("127.0.0.1"));
        cfg.set_master_port(master.split(':').nth(1).unwrap().parse().unwrap());
        //cfg.set_fileserver(fileserver);
        cfg.set_root_dir(root.to_str().unwrap());
        cfg.set_autosync_startup(false);
        cfg.set_reconnect_freq(0);
        cfg.set_reconnect_interval("1");
        cfg
    }

    fn seed_managed_transport(cfg: &MinionConfig, root: &std::path::Path) {
        let (_, master_pbk) = keygen(2048).unwrap();
        key_to_file(&Public(master_pbk.clone()), root.to_str().unwrap(), CFG_MASTER_KEY_PUB).unwrap();

        let keyman = MinionRSAKeyManager::new(root.to_path_buf()).unwrap();
        let minion_pbk = libsysinspect::rsa::keys::from_pem(None, Some(&keyman.get_pubkey_pem()))
            .unwrap()
            .1
            .unwrap();

        TransportStore::for_minion(cfg)
            .unwrap()
            .save(&secure_state(&master_pbk, &minion_pbk))
            .unwrap();
    }

    #[tokio::test]
    async fn reconnect_signal_exits_instance_cleanly() {
        let _guard = TEST_LOCK.lock().await;
        // Fake master
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // Accept connection and just sit there
        tokio::spawn(async move {
            let (sock, _peer) = listener.accept().await.unwrap();
            let _ = shutdown_rx.await;
            drop(sock);
        });

        let tmp = tempfile::tempdir().unwrap();
        let cfg = mk_cfg(
            format!("{}", addr),
            "127.0.0.1:1".to_string(), // not used in this test
            tmp.path(),
        );

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());

        let h = tokio::spawn(async move { _minion_instance(cfg, Some("fp-test".to_string()), dpq).await });

        // Let it start
        tokio::time::sleep(reconnect_boot_wait()).await;

        // Trigger reconnect
        let _ = CONNECTION_TX.send(());
        let _ = shutdown_tx.send(());

        // Must exit quickly
        let res = timeout(reconnect_exit_timeout(), h).await;
        assert!(res.is_ok(), "instance did not exit on reconnect");
    }

    #[tokio::test]
    async fn proto_eof_emits_reconnect_signal() {
        let _guard = TEST_LOCK.lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            drop(sock); // immediate EOF for minion
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip("127.0.0.1");
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());
        cfg.set_autosync_startup(false);
        cfg.set_reconnect_freq(0);
        cfg.set_reconnect_interval("1");

        seed_managed_transport(&cfg, tmp.path());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();

        let mut rx = CONNECTION_TX.subscribe();
        minion.as_ptr().do_proto().await.unwrap();

        // let proto loop actually start
        tokio::time::sleep(Duration::from_millis(200)).await;

        let got = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
        assert!(got.is_ok(), "expected reconnect signal on EOF");
        match got {
            Ok(Ok(_)) => {
                // correct: reconnect signal received
            }
            Ok(Err(e)) => {
                panic!("channel closed unexpectedly: {e}");
            }
            Err(_) => {
                panic!("expected reconnect signal on EOF but timed out");
            }
        }
    }

    #[tokio::test]
    async fn reconnect_does_not_leave_zombie_connection() {
        let _guard = TEST_LOCK.lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Fake master: accept first, drop it, then accept second.
        let (ready_tx, ready_rx) = oneshot::channel();
        let accept2 = tokio::spawn(async move {
            // 1st connect
            let (sock1, _) = listener.accept().await.unwrap();
            tokio::time::sleep(reconnect_drop_wait()).await;
            drop(sock1); // force EOF -> reconnect

            let _ = ready_tx.send(());

            // 2nd connect must happen
            let (_sock2, _) = listener.accept().await.unwrap();
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());
        cfg.set_autosync_startup(false);
        cfg.set_reconnect_freq(0);
        cfg.set_reconnect_interval("1");
        seed_managed_transport(&cfg, tmp.path());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());

        // Run one instance to establish the first connection.
        let h1 = tokio::spawn({
            let cfg = cfg.clone();
            let dpq = dpq.clone();
            async move { _minion_instance(cfg, None, dpq).await }
        });

        // Let the fake master accept, then drop the first connection.
        tokio::time::sleep(reconnect_boot_wait()).await;

        timeout(reconnect_accept_timeout(), ready_rx)
            .await
            .expect("fake master never became ready for second connect")
            .expect("fake master readiness signal dropped");

        match timeout(reconnect_exit_timeout(), h1)
            .await
            .expect("first instance never exited after reconnect")
            .expect("first instance join failed")
        {
            Ok(_) => {}
            Err(SysinspectError::IoErr(err))
                if matches!(
                    err.kind(),
                    ErrorKind::ConnectionReset | ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe | ErrorKind::ConnectionAborted
                ) => {}
            Err(err) => panic!("first instance returned unexpected error: {err}"),
        }

        // Fresh instance must still be able to establish a new TCP session.
        let h2 = tokio::spawn(async move { SysMinion::new(cfg, None, dpq).await });

        // We don't need a full protocol run; just prove the second connect happened.
        timeout(reconnect_accept_timeout(), accept2).await.expect("second connect never happened").unwrap();

        // cleanup
        h2.abort();
        let _ = h2.await;
    }

    #[tokio::test]
    async fn request_writes_len_prefix_and_payload() {
        let _guard = TEST_LOCK.lock().await;
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();

            let mut lenb = [0u8; 4];
            sock.read_exact(&mut lenb).await.unwrap();
            let n = u32::from_be_bytes(lenb) as usize;

            let mut msg = vec![0u8; n];
            sock.read_exact(&mut msg).await.unwrap();

            (n, msg)
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();

        let payload = b"abc123".to_vec();
        minion.request(payload.clone()).await;

        let (n, msg) = server.await.unwrap();
        assert_eq!(n, payload.len());
        assert_eq!(msg, payload);
    }

    #[tokio::test]
    async fn request_seals_payload_when_secure_channel_is_active() {
        let _guard = TEST_LOCK.lock().await;
        use tokio::io::AsyncReadExt;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut lenb = [0u8; 4];
            sock.read_exact(&mut lenb).await.unwrap();
            let mut msg = vec![0u8; u32::from_be_bytes(lenb) as usize];
            sock.read_exact(&mut msg).await.unwrap();
            msg
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();
        let (mut master_channel, minion_channel) = secure_channels(tmp.path());
        minion.set_secure_channel(minion_channel).await;

        minion.request(br#"{"r":"ping"}"#.to_vec()).await;

        assert_eq!(master_channel.open_bytes(&server.await.unwrap()).unwrap(), br#"{"r":"ping"}"#.to_vec());
    }

    #[tokio::test]
    async fn bootstrap_secure_fails_closed_without_trusted_master_key() {
        let _guard = TEST_LOCK.lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (_sock, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();

        let err = minion.bootstrap_secure().await.unwrap_err().to_string();
        assert!(err.contains("Trusted master RSA key is missing"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn bootstrap_secure_marks_transport_state_broken_on_diagnostic_reply() {
        let _guard = TEST_LOCK.lock().await;
        use libsysproto::secure::{SecureBootstrapDiagnostic, SecureDiagnosticCode, SecureFailureSemantics, SecureFrame};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut lenb = [0u8; 4];
            sock.read_exact(&mut lenb).await.unwrap();
            let mut hello = vec![0u8; u32::from_be_bytes(lenb) as usize];
            sock.read_exact(&mut hello).await.unwrap();
            assert!(!hello.is_empty());

            let reply = serde_json::to_vec(&SecureFrame::BootstrapDiagnostic(SecureBootstrapDiagnostic {
                code: SecureDiagnosticCode::BootstrapRejected,
                message: "stale trust".to_string(),
                failure: SecureFailureSemantics::diagnostic(false, true),
            }))
            .unwrap();
            sock.write_all(&(reply.len() as u32).to_be_bytes()).await.unwrap();
            sock.write_all(&reply).await.unwrap();
            sock.flush().await.unwrap();
        });

        let tmp = tempfile::tempdir().unwrap();
        let (_, master_pbk) = keygen(2048).unwrap();
        key_to_file(&Public(master_pbk), tmp.path().to_str().unwrap(), CFG_MASTER_KEY_PUB).unwrap();

        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg.clone(), None, dpq).await.unwrap();
        let store = TransportStore::for_minion(&cfg).unwrap();
        let err = minion.bootstrap_secure().await.unwrap_err().to_string();
        assert!(err.contains("Master rejected secure bootstrap"), "unexpected error: {err}");

        let state = store.load().unwrap().unwrap();
        assert!(state.active_key_id.is_none());
        assert!(state.last_key_id.is_some());
        assert_eq!(state.keys.iter().filter(|record| record.status == TransportKeyStatus::Broken).count(), 1);
    }

    #[tokio::test]
    async fn send_ehlo_includes_sid_and_is_jsonish() {
        let _guard = TEST_LOCK.lock().await;
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();

            let mut lenb = [0u8; 4];
            sock.read_exact(&mut lenb).await.unwrap();
            let n = u32::from_be_bytes(lenb) as usize;

            let mut msg = vec![0u8; n];
            sock.read_exact(&mut msg).await.unwrap();
            msg
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();

        minion.as_ptr().send_ehlo().await.unwrap();

        let msg = server.await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&msg).unwrap();

        assert_eq!(v["r"], "ehlo");
        assert_eq!(v["sid"], MINION_SID.to_string());
        assert!(v["d"].is_object());
    }

    #[tokio::test]
    async fn send_traits_writes_a_message_over_wire() {
        let _guard = TEST_LOCK.lock().await;
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();

            let mut lenb = [0u8; 4];
            sock.read_exact(&mut lenb).await.unwrap();
            let n = u32::from_be_bytes(lenb) as usize;

            let mut msg = vec![0u8; n];
            sock.read_exact(&mut msg).await.unwrap();
            msg
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();

        minion.as_ptr().send_traits().await.unwrap();

        let msg = server.await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&msg).unwrap();

        assert_eq!(v["r"], "tr");
        assert_eq!(v["sid"], MINION_SID.to_string());
        assert!(v["d"].is_object());
    }

    #[tokio::test]
    async fn send_sensors_sync_emits_expected_r_code() {
        let _guard = TEST_LOCK.lock().await;
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut lenb = [0u8; 4];
            sock.read_exact(&mut lenb).await.unwrap();
            let n = u32::from_be_bytes(lenb) as usize;
            let mut msg = vec![0u8; n];
            sock.read_exact(&mut msg).await.unwrap();
            msg
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();

        minion.as_ptr().send_sensors_sync().await.unwrap();

        let msg = server.await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&msg).unwrap();
        eprintln!("sensors_sync msg = {}", v); // one-time debug

        assert_eq!(v["r"], "ssr");

        // sid might be missing for this message; assert it’s at least present OR intentionally absent
        assert!(v.get("sid").is_some(), "sensors sync message missing sid field: {v}");
    }

    #[tokio::test]
    async fn stop_background_aborts_and_clears_handles() {
        let _guard = TEST_LOCK.lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (_sock, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(60)).await;
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();

        // inject dummy tasks
        let h1 = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(60)).await });
        let h2 = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(60)).await });
        let h3 = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(60)).await });

        *minion.ping_task.lock().await = Some(h1);
        *minion.proto_task.lock().await = Some(h2);
        *minion.stats_task.lock().await = Some(h3);

        minion.stop_background().await;

        assert!(minion.ping_task.lock().await.is_none());
        assert!(minion.proto_task.lock().await.is_none());
        assert!(minion.stats_task.lock().await.is_none());
    }

    #[tokio::test]
    async fn stop_sensors_aborts_and_clears_handles() {
        let _guard = TEST_LOCK.lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (_sock, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(60)).await;
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();

        let h1 = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(60)).await });
        let h2 = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(60)).await });

        *minion.sensors_pump.lock().await = Some(h1);
        *minion.sensors_task.lock().await = Some(h2);

        minion.stop_sensors().await;

        assert!(minion.sensors_pump.lock().await.is_none());
        assert!(minion.sensors_task.lock().await.is_none());
    }

    #[tokio::test]
    async fn ping_watchdog_triggers_reconnect_after_timeout() {
        let _guard = TEST_LOCK.lock().await;
        use tokio::time::timeout;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (_sock, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(60)).await;
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let mut minion = Arc::try_unwrap(SysMinion::new(cfg, None, dpq).await.unwrap()).ok().unwrap();
        minion.set_ping_timeout(Duration::from_millis(300));

        let minion = Arc::new(minion);
        let mut rx = CONNECTION_TX.subscribe();
        minion.as_ptr().do_ping_update(Arc::new(ExitState::new())).await.unwrap();

        let res = timeout(Duration::from_secs(2), rx.recv()).await;
        assert!(res.is_ok(), "watchdog did not trigger reconnect");
    }

    #[tokio::test]
    async fn update_ping_resets_timeout() {
        let _guard = TEST_LOCK.lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (_sock, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(60)).await;
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let mut minion = Arc::try_unwrap(SysMinion::new(cfg, None, dpq).await.unwrap()).ok().unwrap();
        minion.set_ping_timeout(Duration::from_millis(300));

        let minion = Arc::new(minion);
        minion.update_ping().await;
        tokio::time::sleep(Duration::from_millis(150)).await;
        minion.update_ping().await;

        let elapsed = minion.last_ping.lock().await.elapsed();
        assert!(elapsed < Duration::from_millis(200));
    }

    #[tokio::test]
    async fn request_handles_closed_socket_gracefully() {
        let _guard = TEST_LOCK.lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            drop(sock); // close immediately
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();

        // Should not panic
        minion.request(b"abc".to_vec()).await;
    }

    #[tokio::test]
    async fn bootstrap_secure_fails_on_truncated_reply_frame() {
        let _guard = TEST_LOCK.lock().await;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut lenb = [0u8; 4];
            sock.read_exact(&mut lenb).await.unwrap();
            let mut hello = vec![0u8; u32::from_be_bytes(lenb) as usize];
            sock.read_exact(&mut hello).await.unwrap();
            assert!(!hello.is_empty());

            sock.write_all(&32u32.to_be_bytes()).await.unwrap();
            sock.write_all(br#"{"kind":"bootstrap_ack""#).await.unwrap();
            sock.flush().await.unwrap();
        });

        let tmp = tempfile::tempdir().unwrap();
        let (_, master_pbk) = keygen(2048).unwrap();
        key_to_file(&Public(master_pbk), tmp.path().to_str().unwrap(), CFG_MASTER_KEY_PUB).unwrap();

        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let minion = SysMinion::new(cfg, None, dpq).await.unwrap();
        let err = minion.bootstrap_secure().await.unwrap_err().to_string();

        assert!(err.contains("decode secure bootstrap reply") || err.contains("early eof"));
    }

    #[tokio::test]
    async fn repeated_reconnect_signals_do_not_hang_instance_shutdown() {
        let _guard = TEST_LOCK.lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let _ = shutdown_rx.await;
            drop(sock);
        });

        let tmp = tempfile::tempdir().unwrap();
        let cfg = mk_cfg(format!("{addr}"), "127.0.0.1:1".to_string(), tmp.path());
        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());
        let handle = tokio::spawn(async move { _minion_instance(cfg, Some("fp-test".to_string()), dpq).await });

        tokio::time::sleep(Duration::from_millis(200)).await;
        let _ = CONNECTION_TX.send(());
        let _ = CONNECTION_TX.send(());
        let _ = CONNECTION_TX.send(());
        let _ = shutdown_tx.send(());

        assert!(timeout(Duration::from_secs(2), handle).await.is_ok(), "instance did not exit under reconnect storm");
    }
}
