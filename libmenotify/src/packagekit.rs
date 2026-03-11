use crate::MeNotifyError;
use serde::Serialize;
use std::collections::BTreeMap;
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::OwnedValue,
};

const PK_DEST: &str = "org.freedesktop.PackageKit";
const PK_PATH: &str = "/org/freedesktop/PackageKit";
const PK_IFACE: &str = "org.freedesktop.PackageKit";

/// PackageKit helper API for MeNotify.
pub struct MeNotifyPackageKit;

impl MeNotifyPackageKit {
    /// Returns true if PackageKit can be reached on the system bus.
    ///
    /// Returns:
    /// * `true` if the root PackageKit proxy can be created.
    /// * `false` otherwise.
    pub fn available() -> bool {
        Self::root_proxy().is_ok()
    }

    /// Returns a summary of the current PackageKit daemon state.
    ///
    /// Returns:
    /// * `Ok(PackageKitStatus)` with daemon properties and active transactions.
    /// * `Err(MeNotifyError)` if PackageKit is unavailable or D-Bus access fails.
    pub fn status() -> Result<PackageKitStatus, MeNotifyError> {
        let proxy = Self::root_proxy()?;
        Ok(PackageKitStatus {
            available: true,
            backend_name: proxy.get_property("BackendName")?,
            distro_id: proxy.get_property("DistroId")?,
            daemon_state: proxy.call("GetDaemonState", &())?,
            locked: proxy.get_property("Locked")?,
            network_state: proxy.get_property("NetworkState")?,
            version_major: proxy.get_property("VersionMajor")?,
            version_micro: proxy.get_property("VersionMicro")?,
            version_minor: proxy.get_property("VersionMinor")?,
            transactions: proxy.call("GetTransactionList", &())?,
        })
    }

    /// Returns PackageKit package history for the requested package names.
    ///
    /// Arguments:
    /// * `names` - Package names to query.
    /// * `count` - Maximum number of history entries per package.
    ///
    /// Returns:
    /// * `Ok(serde_json::Value)` containing the raw PackageKit history structure.
    /// * `Err(MeNotifyError)` if D-Bus access or JSON conversion fails.
    pub fn history(names: Vec<String>, count: u32) -> Result<serde_json::Value, MeNotifyError> {
        let proxy = Self::root_proxy()?;
        let data = proxy.call::<_, _, BTreeMap<String, Vec<BTreeMap<String, OwnedValue>>>>("GetPackageHistory", &(names, count))?;
        serde_json::to_value(data).map_err(|err| MeNotifyError::PackageKit(format!("failed to convert package history to JSON: {err}")))
    }

    fn root_proxy() -> Result<Proxy<'static>, MeNotifyError> {
        Ok(Proxy::new(&Connection::system()?, PK_DEST, PK_PATH, PK_IFACE)?)
    }
}

/// Serializable snapshot of the PackageKit root daemon state.
#[derive(Debug, Serialize)]
pub struct PackageKitStatus {
    available: bool,
    backend_name: String,
    daemon_state: String,
    distro_id: String,
    locked: bool,
    network_state: u32,
    transactions: Vec<String>,
    version_major: u32,
    version_micro: u32,
    version_minor: u32,
}
