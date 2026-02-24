use crate::resources::DataItemMeta;
use libc::{AT_FDCWD, timespec, utimensat};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Returns the current Unix timestamp in seconds.
pub(crate) fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0)).as_secs()
}

/// Computes the SHA256 hash of a file and returns it as a hexadecimal string.
pub(crate) fn get_sha256(p: &Path) -> io::Result<String> {
    let mut f = fs::File::open(p)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Atomically copy a file from src to dst. The dst file will be replaced if it exists.
/// This is used to ensure that we never have a partially-written file at dst, even if the process is killed during the copy.
pub(crate) fn copy(src: &Path, dst: &Path) -> io::Result<()> {
    let tmp = dst.with_extension("tmp");
    fs::copy(src, &tmp)?;
    fs::rename(tmp, dst)?;
    Ok(())
}

/// Writes a serializable value to a JSON file at the specified path.
/// The write is atomic: the data is written to a temporary file first, then renamed to the target path.
/// This ensures that the target file is never left in a partially-written state.
pub(crate) fn json_write<T: Serialize>(pth: &Path, v: &T) -> io::Result<()> {
    let tmp = pth.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(v).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    fs::rename(tmp, pth)?;
    Ok(())
}

/// Recursively walks through a directory tree and collects all `.meta.json` files.
/// Returns a vector of paths to all metadata files found, or an error if the directory cannot be read.
pub(crate) fn meta_tree(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = vec![];
    if !root.exists() {
        return Ok(out);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for ent in fs::read_dir(&dir)? {
            let ent = ent?;
            let p = ent.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|s| s.to_str()) == Some("json")
                && p.file_name().and_then(|s| s.to_str()).unwrap_or("").ends_with(".meta.json")
            {
                out.push(p);
            }
        }
    }
    Ok(out)
}

/// Recursively walks through a directory tree and collects all binary data files (`.bin` extension).
/// Returns a vector of paths to all data files found, or an error if the directory cannot be read.
pub(crate) fn data_tree(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = vec![];
    if !root.exists() {
        return Ok(out);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for ent in fs::read_dir(&dir)? {
            let ent = ent?;
            let p = ent.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|s| s.to_str()) == Some("bin") {
                out.push(p);
            }
        }
    }
    Ok(out)
}

/// Applies the file mode and timestamps from the metadata to the specified file path.
///
/// Example usage:
/// ```
/// let meta = ds.meta(&sha)?.unwrap();
/// fs::copy(ds.uri(&sha), "/your/bin")?;
/// ds.set_file_attrs(&meta, "/your/bin")?;
/// ```
pub fn set_file_attrs(meta: &DataItemMeta, dst: impl AsRef<Path>) -> io::Result<()> {
    let dst = dst.as_ref();
    fs::set_permissions(dst, fs::Permissions::from_mode(meta.fmode & 0o7777))?;

    unsafe {
        if utimensat(
            AT_FDCWD,
            CString::new(dst.as_os_str().as_bytes())?.as_ptr(),
            [timespec { tv_sec: meta.created_unix as i64, tv_nsec: 0 }, timespec { tv_sec: meta.created_unix as i64, tv_nsec: 0 }].as_ptr(),
            0,
        ) != 0
        {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}
