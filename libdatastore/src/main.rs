use libdatastore::{cfg::DataStorageConfig, resources::DataStorage};
use std::{env, time::Duration};

const GIB: u64 = 1024 * 1024 * 1024;

fn main() -> anyhow::Result<()> {
    let cfg = DataStorageConfig::new().expiration(Duration::from_secs(3 * 24 * 60 * 60)).max_item_size(GIB).max_overall_size(20 * GIB);

    let storage = DataStorage::new(cfg, "/tmp/store")?;

    let exe = env::current_exe()?;
    let exe_str = exe.to_str().ok_or_else(|| anyhow::anyhow!("current_exe() path is not valid UTF-8: {:?}", exe))?;

    let item = storage.add(exe_str)?;
    let _meta = storage.meta(&item.sha256)?;

    Ok(())
}
