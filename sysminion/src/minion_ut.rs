#[cfg(test)]
mod tests {
    use crate::minion::{_minion_instance, SysMinion};
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

        let got = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
        assert!(got.is_ok(), "expected reconnect signal on EOF");
    }
}
