use crate::{
    cfg::DataStorageConfig,
    util::{copy, data_tree, get_sha256, json_write, meta_tree, unix_now},
};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::MetadataExt;
use std::{
    fs,
    io::{self},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct DataStorage {
    cfg: DataStorageConfig,
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataItemMeta {
    pub sha256: String,
    pub size_bytes: u64,
    pub created_unix: u64,
    pub expires_unix: Option<u64>,
    pub fname: Option<String>,
    pub fmode: u32,
}

impl DataStorage {
    pub fn new(cfg: DataStorageConfig, root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(Self { cfg, root })
    }

    /// Add a file to the store (copy). Returns metadata.
    pub fn add(&self, src: impl AsRef<Path>) -> io::Result<DataItemMeta> {
        let src = src.as_ref();
        let md = fs::metadata(src)?;

        let unix_mode = (md.mode() & 0o7777) as u32;
        if !md.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "src is not a file"));
        }

        let size = md.len();
        if let Some(max) = self.cfg.get_max_item_size()
            && size > max {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("item too big: {size} > {max} bytes")));
            }

        // Ensure overall limit BEFORE writing (best-effort).
        if let Some(max_total) = self.cfg.get_max_overall_size() {
            let total = self.total()?;
            if total.saturating_add(size) > max_total {
                // try GC once; if still too big, reject.
                self.gc()?;
                let total2 = self.total()?;
                if total2.saturating_add(size) > max_total {
                    return Err(io::Error::new(io::ErrorKind::OutOfMemory, format!("storage full: {total2}+{size} > {max_total} bytes")));
                }
            }
        }

        // Hash streaming
        let sha256 = get_sha256(src)?;
        let (dir, data_path, meta_path) = self.shardpath(&sha256);

        fs::create_dir_all(&dir)?;

        // If data already exists, do not rewrite it
        if data_path.exists() {
            let have = get_sha256(&data_path)?;
            if have != sha256 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("store corruption: expected {sha256}, got {have} at {data_path:?}")));
            }
        } else {
            copy(src, &data_path)?;
        }

        let now = unix_now();
        let expires = self.cfg.get_expiration().map(|d| now.saturating_add(d.as_secs()));

        let meta = DataItemMeta {
            sha256: sha256.clone(),
            size_bytes: size,
            created_unix: now,
            expires_unix: expires,
            fname: src.file_name().map(|s| s.to_string_lossy().to_string()),
            fmode: unix_mode,
        };

        // Write meta last (so presence of meta implies object is ready).
        json_write(&meta_path, &meta)?;

        Ok(meta)
    }

    /// Read metadata for an object, if it exists.
    pub fn meta(&self, sha256: &str) -> io::Result<Option<DataItemMeta>> {
        let (_dir, _data_path, meta_path) = self.shardpath(sha256);
        if !meta_path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&meta_path)?;
        let meta: DataItemMeta = serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        Ok(Some(meta))
    }

    /// Return the on-disk path for download/serve.
    pub fn uri(&self, sha256: &str) -> PathBuf {
        let (_dir, data_path, _meta_path) = self.shardpath(sha256);
        data_path
    }

    /// Garbage-collect expired items, then enforce max_overall_size by oldest-first.
    pub fn gc(&self) -> io::Result<()> {
        self.expire()?;

        if let Some(max_total) = self.cfg.get_max_overall_size() {
            loop {
                let total = self.total()?;
                if total <= max_total {
                    break;
                }

                // Delete oldest meta+data pair.
                if let Some(oldest) = self.oldest()? {
                    self.del(&oldest.sha256)?;
                } else {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Deletes an object and its metadata from the storage. Best-effort: if files are missing, ignore and continue.
    pub fn del(&self, sha256: &str) -> io::Result<()> {
        let (_dir, data_path, meta_path) = self.shardpath(sha256);
        match fs::remove_file(&meta_path) {
            Ok(_) => (),
            Err(e) => {
                // only log error, best effort.
                log::error!("Failed to remove meta file {meta_path:?}: {e}");
            }
        }
        match fs::remove_file(&data_path) {
            Ok(_) => (),
            Err(e) => {
                // only log error, best effort.
                log::error!("Failed to remove data file {data_path:?}: {e}");
            }
        }
        Ok(())
    }

    // Shard: aa/bb/<fullsha>.(bin|json)
    fn shardpath(&self, sha256: &str) -> (PathBuf, PathBuf, PathBuf) {
        let a = &sha256.get(0..2).unwrap_or("xx");
        let b = &sha256.get(2..4).unwrap_or("yy");

        let dir = self.root.join(a).join(b);
        let data_path = dir.join(format!("{sha256}.bin"));
        let meta_path = dir.join(format!("{sha256}.meta.json"));
        (dir, data_path, meta_path)
    }

    fn expire(&self) -> io::Result<()> {
        let now = unix_now();
        for meta_path in meta_tree(&self.root)? {
            if let Ok(bytes) = fs::read(&meta_path)
                && let Ok(meta) = serde_json::from_slice::<DataItemMeta>(&bytes)
                    && let Some(exp) = meta.expires_unix
                        && exp <= now {
                            self.del(&meta.sha256)?;
                        }
        }
        Ok(())
    }

    fn oldest(&self) -> io::Result<Option<DataItemMeta>> {
        let mut best: Option<DataItemMeta> = None;

        for meta_path in meta_tree(&self.root)? {
            let bytes = match fs::read(&meta_path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let meta: DataItemMeta = match serde_json::from_slice(&bytes) {
                Ok(m) => m,
                Err(_) => continue,
            };

            best = match best {
                None => Some(meta),
                Some(cur) => {
                    if meta.created_unix < cur.created_unix {
                        Some(meta)
                    } else {
                        Some(cur)
                    }
                }
            };
        }

        Ok(best)
    }

    /// Computes the total size of all data files in bytes.
    fn total(&self) -> io::Result<u64> {
        let mut total = 0u64;
        for data_path in data_tree(&self.root)? {
            if let Ok(md) = fs::metadata(&data_path) {
                total = total.saturating_add(md.len());
            }
        }
        Ok(total)
    }
}
