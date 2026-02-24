use crate::resources::DataItemMeta;
use crate::util::set_file_attrs;
use futures_util::StreamExt;
use reqwest::Client;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// A simple client for uploading files to the datastore via HTTP API.
/// This is a basic example and can be extended with error handling, authentication, etc.
/// Example usage:
/// ```
/// upload_artefact("http://localhost:8080", "/path/to/file").await?;
/// ```
pub async fn upload_artefact(master_url: &str, path: &Path) -> anyhow::Result<()> {
    let client = Client::new();
    let bytes = fs::read(path).await?;

    let resp = client
        .post(format!("{master_url}/store"))
        .header("Content-Type", "application/octet-stream")
        .header("X-Filename", path.to_string_lossy().to_string())
        .body(bytes)
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Upload failed: {}", resp.text().await?);
    }

    let json: serde_json::Value = resp.json().await?;
    log::debug!("Upload response: {json:#}");

    Ok(())
}

/// A simple client for downloading files from the datastore via HTTP API.
/// This is a basic example and can be extended with error handling, authentication, etc.
/// Example usage:
/// ```
/// download_artefact("http://localhost:8080", "sha256hash").await?;
/// ```
pub async fn download_artefact(master_url: &str, sha256: &str) -> anyhow::Result<()> {
    let client = Client::new();

    let meta: serde_json::Value = client.get(format!("{master_url}/store/{sha256}")).send().await?.json().await?;
    let target_path = meta["fname"].as_str().ok_or_else(|| anyhow::anyhow!("No fname in metadata"))?;
    let bytes = client.get(format!("{master_url}/store/{sha256}/blob")).send().await?.bytes().await?;
    let path = Path::new(target_path);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut file = fs::File::create(path).await?;
    file.write_all(&bytes).await?;
    file.flush().await?;
    drop(file);

    if let Some(mode) = meta["fmode"].as_u64() {
        let perms = std::fs::Permissions::from_mode(mode as u32);
        std::fs::set_permissions(path, perms)?;
    }

    log::debug!("Restored to {}", target_path);

    Ok(())
}

/// Stream-download a blob from the master and write it atomically to `dst`.
/// - No buffering whole file in memory
/// - Writes to `dst.tmp` then renames
///
/// Example usage:
/// ```
/// atomic_download("http://master/store/sha256hash/blob", "/your/bin").await?;
/// ```
pub async fn atomic_download(url: &str, dst: impl AsRef<Path>) -> io::Result<()> {
    let dst = dst.as_ref();
    let tmp = dst.with_extension("tmp");

    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let client = reqwest::Client::new();
    let resp = client.get(url).send().await.map_err(|e| io::Error::other(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(io::Error::other(format!("HTTP {status}: {body}")));
    }

    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| io::Error::other(e.to_string()))?;
        file.write_all(&chunk).await?;
    }

    file.flush().await?;
    drop(file);

    tokio::fs::rename(&tmp, dst).await?;
    Ok(())
}

/// Download a blob by its original filename. This is a convenience function that first resolves the filename to a SHA256 hash, then downloads the blob by hash.
/// This is not atomic by itself, but relies on the underlying `atomic_download` to ensure atomicity of the file write.
/// Example usage:
/// ```
/// let meta = download_by_name("http://localhost:8080", "myfile.txt").await?;
/// ```
pub async fn download_by_name(master: &str, fname: &str) -> io::Result<DataItemMeta> {
    let client = reqwest::Client::new();
    let url = format!("{master}/store/resolve?fname={}", urlencoding::encode(fname));

    let meta: DataItemMeta = client
        .get(url)
        .send()
        .await
        .map_err(|e| io::Error::other(e.to_string()))?
        .json()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let blob_url = format!("{master}/store/{}/blob", meta.sha256);

    atomic_download(&blob_url, fname).await?;
    set_file_attrs(&meta, fname)?;

    Ok(meta)
}
