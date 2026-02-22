#[cfg(test)]
mod tests {
    use crate::minion::{_minion_instance, MINION_SID, SysMinion};
    use crate::proto::msg::CONNECTION_TX;
    use libdpq::DiskPersistentQueue;
    use libsysinspect::cfg::mmconf::MinionConfig;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, timeout};

    fn mk_cfg(master: String, fileserver: String, root: &std::path::Path) -> MinionConfig {
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(master.split(':').next().unwrap_or("127.0.0.1"));
        cfg.set_master_port(master.split(':').nth(1).unwrap().parse().unwrap());
        //cfg.set_fileserver(fileserver);
        cfg.set_root_dir(root.to_str().unwrap());
        cfg.set_reconnect_freq(0);
        cfg.set_reconnect_interval("1");
        cfg
    }

    #[tokio::test]
    async fn reconnect_signal_exits_instance_cleanly() {
        // Fake master
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Accept connection and just sit there
        tokio::spawn(async move {
            let (_sock, _peer) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(60)).await;
        });

        let tmp = tempfile::tempdir().unwrap();
        let cfg = mk_cfg(
            format!("{}", addr),
            "127.0.0.1:1".to_string(), // not used in this test
            tmp.path(),
        );

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());

        let h = tokio::spawn(async move { _minion_instance(cfg, None, dpq).await });

        // Let it start
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Trigger reconnect
        let _ = CONNECTION_TX.send(());

        // Must exit quickly
        let res = timeout(Duration::from_secs(2), h).await;
        assert!(res.is_ok(), "instance did not exit on reconnect");
    }

    #[tokio::test]
    async fn proto_eof_emits_reconnect_signal() {
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
        cfg.set_reconnect_freq(0);
        cfg.set_reconnect_interval("1");

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
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Fake master: accept first, drop it, then accept second.
        let accept2 = tokio::spawn(async move {
            // 1st connect
            let (sock1, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_millis(150)).await;
            drop(sock1); // force EOF -> reconnect

            // 2nd connect must happen
            let (_sock2, _) = listener.accept().await.unwrap();
        });

        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_master_ip(&addr.ip().to_string());
        cfg.set_master_port(addr.port().into());
        cfg.set_root_dir(tmp.path().to_str().unwrap());
        cfg.set_reconnect_freq(0);
        cfg.set_reconnect_interval("1");

        let dpq = Arc::new(DiskPersistentQueue::open(tmp.path().join("pending-tasks")).unwrap());

        // Run one instance, it should exit on EOF because do_proto sends reconnect.
        let h1 = tokio::spawn({
            let cfg = cfg.clone();
            let dpq = dpq.clone();
            async move { _minion_instance(cfg, None, dpq).await }
        });

        // Wait for first to end
        let _ = timeout(Duration::from_secs(3), h1).await.expect("first instance did not exit");

        // Run second instance; must be able to connect (fake master is waiting)
        let h2 = tokio::spawn(async move { _minion_instance(cfg, None, dpq).await });

        // We don't need it to finish; just prove it connected by allowing accept2 to finish.
        timeout(Duration::from_secs(3), accept2).await.expect("second connect never happened").unwrap();

        // cleanup
        h2.abort();
        let _ = h2.await;
    }

    #[tokio::test]
    async fn request_writes_len_prefix_and_payload() {
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
    async fn send_ehlo_includes_sid_and_is_jsonish() {
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
}
