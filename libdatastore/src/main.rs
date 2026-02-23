use libdatastore::{cfg::DataStorageConfig, resources::DataStorage};
use std::env;

fn main() -> anyhow::Result<()> {
    let cfg = DataStorageConfig::new().expiration("3 days")?.max_item_size("1 gb")?.max_overall_size("20 gb")?;

    let storage = DataStorage::new(cfg, "/tmp/store")?;
    let _same = storage.meta(&storage.add(env::current_exe()?.to_str().unwrap())?.sha256)?;
    Ok(())
}
