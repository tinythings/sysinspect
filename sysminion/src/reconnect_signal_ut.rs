use once_cell::sync::Lazy;
use tokio::sync::broadcast;

static TEST_RECONNECT_TX: Lazy<broadcast::Sender<()>> = Lazy::new(|| {
    let (tx, _) = broadcast::channel(32);
    tx
});

pub(crate) fn notify_reconnect_signal() {
    let _ = TEST_RECONNECT_TX.send(());
}

pub(crate) fn subscribe_reconnect_signal() -> broadcast::Receiver<()> {
    TEST_RECONNECT_TX.subscribe()
}
