use libdpq::{DiskPersistentQueue, WorkItem};
use libsysproto::{MasterMessage, MinionTarget, rqtypes::RequestType};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let q = DiskPersistentQueue::open("/tmp/libdpq-demo-queue")?;
    let q2 = q.clone();
    q.start(move |job_id, item| {
        let q3 = q2.clone();
        async move {
            match item {
                WorkItem::MasterCommand(msg) => {
                    println!("JOB {job_id}: got MasterMessage: {msg:#?}");
                }
            }

            if let Err(e) = q3.ack(job_id) {
                eprintln!("Ack failed for {job_id}: {e}");
            }
        }
    });

    // Create a sample MasterMessage and enqueue it
    let mut target = MinionTarget::new("DEADBEEF", "137");
    target.add_hostname("beispiel.de");

    let mut msg = MasterMessage::new(RequestType::Ping, json!("Hello from DPQ demo!"));
    msg.set_target(target);

    let id = q.add(WorkItem::MasterCommand(msg))?;
    println!("Enqueued job id={id}");

    // Keep alive so runner can run
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
