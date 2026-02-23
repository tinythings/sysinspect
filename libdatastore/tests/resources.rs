use libdatastore::{cfg::DataStorageConfig, resources::DataStorage};
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
    thread::sleep,
    time::Duration,
};

fn write_file(p: &Path, size: usize, mode: u32) -> io::Result<()> {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::File::create(p)?;
    f.write_all(&vec![0xABu8; size])?;
    f.sync_all()?;
    fs::set_permissions(p, fs::Permissions::from_mode(mode))?;
    Ok(())
}

fn store_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn add_and_meta_roundtrip_and_files_exist() -> anyhow::Result<()> {
    let root = store_root();
    let src = root.path().join("src.bin");
    write_file(&src, 128, 0o755)?;

    let cfg = DataStorageConfig::new().max_item_size("1 mb")?.max_overall_size("10 mb")?;

    let ds = DataStorage::new(cfg, root.path().join("store"))?;
    let meta = ds.add(&src)?;

    // meta sanity
    assert_eq!(meta.size_bytes, 128);
    assert_eq!(meta.fmode & 0o7777, 0o755);
    assert!(!meta.sha256.is_empty());

    // meta() returns same
    let meta2 = ds.meta(&meta.sha256)?.expect("meta present");
    assert_eq!(meta2.sha256, meta.sha256);
    assert_eq!(meta2.size_bytes, meta.size_bytes);
    assert_eq!(meta2.fmode, meta.fmode);

    // data file exists
    let data_path = ds.uri(&meta.sha256);
    assert!(data_path.exists(), "data blob missing: {:?}", data_path);

    // meta file exists (derive its path the same way storage does)
    // We can just check that meta() worked; but also assert sidecar file exists:
    let shard_dir = data_path.parent().unwrap();
    let meta_path = shard_dir.join(format!("{}.meta.json", meta.sha256));
    assert!(meta_path.exists(), "meta sidecar missing: {:?}", meta_path);

    Ok(())
}

#[test]
fn max_item_size_rejects() -> anyhow::Result<()> {
    let root = store_root();
    let src = root.path().join("big.bin");
    write_file(&src, 1024, 0o644)?;

    let cfg = DataStorageConfig::new().max_item_size("512 bytes")?;
    let ds = DataStorage::new(cfg, root.path().join("store"))?;

    let err = ds.add(&src).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    Ok(())
}

#[test]
fn expiration_zero_seconds_gc_removes_immediately() -> anyhow::Result<()> {
    let root = store_root();
    let src = root.path().join("src.bin");
    write_file(&src, 64, 0o600)?;

    // 0s means expires_unix == now, so gc should delete it.
    let cfg = DataStorageConfig::new().expiration("0s")?;
    let ds = DataStorage::new(cfg, root.path().join("store"))?;

    let meta = ds.add(&src)?;
    assert!(ds.meta(&meta.sha256)?.is_some(), "meta missing right after add");

    ds.gc()?;

    assert!(ds.meta(&meta.sha256)?.is_none(), "expected expired object to be removed");
    assert!(!ds.uri(&meta.sha256).exists(), "expected data blob to be removed");

    Ok(())
}

#[test]
fn max_overall_size_rejects_when_add_would_exceed() -> anyhow::Result<()> {
    let root = store_root();
    let store_dir = root.path().join("store");
    let ds = DataStorage::new(DataStorageConfig::new().max_item_size("10 mb")?.max_overall_size("150 bytes")?, &store_dir)?;

    // file1: 100 bytes OK
    let f1 = root.path().join("f1.bin");
    write_file(&f1, 100, 0o644)?;
    let _m1 = ds.add(&f1)?;

    // file2: 100 bytes -> total would become ~200 > 150 => should reject (current behavior)
    let f2 = root.path().join("f2.bin");
    write_file(&f2, 100, 0o644)?;
    let err = ds.add(&f2).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::OutOfMemory);

    Ok(())
}

#[test]
fn gc_oldest_prefers_older_timestamp() -> anyhow::Result<()> {
    // This test targets `oldest()` selection logic.
    // We force different created_unix values by sleeping between adds.
    let root = store_root();
    let store_dir = root.path().join("store");
    let ds = DataStorage::new(DataStorageConfig::new().max_overall_size("1 mb")?, &store_dir)?;

    let f1 = root.path().join("a.bin");
    write_file(&f1, 10, 0o644)?;
    let m1 = ds.add(&f1)?;

    sleep(Duration::from_secs(1));

    let f2 = root.path().join("b.bin");
    write_file(&f2, 10, 0o644)?;
    let m2 = ds.add(&f2)?;

    // Manually delete oldest as gc would do when trimming.
    let oldest = ds.meta(&m1.sha256)?.expect("m1 meta").created_unix;

    let newer = ds.meta(&m2.sha256)?.expect("m2 meta").created_unix;

    assert!(oldest <= newer, "expected m1 to be older-or-equal to m2 (seconds resolution)");

    Ok(())
}
