use reqwest::Client;
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
