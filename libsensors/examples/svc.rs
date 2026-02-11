use libsensors::{load, service::SensorService};
use std::path::Path;

#[tokio::main]
async fn main() {
    env_logger::init();
    let spec = load(Path::new(".")).unwrap();
    let svc = SensorService::new(spec);

    let _handles = svc.start();

    // keep process alive
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
