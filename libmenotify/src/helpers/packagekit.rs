use crate::MeNotifyError;
use serde::Serialize;
use std::collections::BTreeMap;
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::{OwnedObjectPath, OwnedValue},
};

const PK_DEST: &str = "org.freedesktop.PackageKit";
const PK_PATH: &str = "/org/freedesktop/PackageKit";
const PK_IFACE: &str = "org.freedesktop.PackageKit";
const PK_TX_IFACE: &str = "org.freedesktop.PackageKit.Transaction";

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

    /// Returns a snapshot of installed packages currently known to PackageKit.
    ///
    /// Returns:
    /// * `Ok(serde_json::Value)` containing an array of installed package objects.
    /// * `Err(MeNotifyError)` if D-Bus access or transaction processing fails.
    pub fn packages() -> Result<serde_json::Value, MeNotifyError> {
        serde_json::to_value(Self::collect_installed_packages()?)
            .map_err(|err| MeNotifyError::PackageKit(format!("failed to convert installed package list to JSON: {err}")))
    }

    fn root_proxy() -> Result<Proxy<'static>, MeNotifyError> {
        Ok(Proxy::new(&Connection::system()?, PK_DEST, PK_PATH, PK_IFACE)?)
    }

    fn collect_installed_packages() -> Result<Vec<PackageKitPackage>, MeNotifyError> {
        let conn = Connection::system()?;
        let root = Proxy::new(&conn, PK_DEST, PK_PATH, PK_IFACE)?;
        let tx_path = root.call::<_, _, OwnedObjectPath>("CreateTransaction", &())?;
        let tx = Proxy::new(&conn, PK_DEST, tx_path.as_str(), PK_TX_IFACE)?;
        let mut signals = tx.receive_all_signals()?;
        let mut packages = Vec::new();

        tx.call::<_, _, ()>("GetPackages", &(0u64,))?;

        for msg in &mut signals {
            match msg.header().member().map(|m| m.as_str()) {
                Some("Package") => {
                    let (info, package_id, summary) = msg.body().deserialize::<(u32, String, String)>()?;
                    if let Some(pkg) = PackageKitPackage::from_signal(info, &package_id, &summary) {
                        packages.push(pkg);
                    }
                }
                Some("ErrorCode") => {
                    let (code, details) = msg.body().deserialize::<(u32, String)>()?;
                    return Err(MeNotifyError::PackageKit(format!("transaction failed with error code {code}: {details}")));
                }
                Some("Finished") => break,
                _ => (),
            }
        }

        Ok(packages)
    }
}

/// Serializable snapshot of the PackageKit root daemon state.
#[derive(Debug, Serialize)]
pub struct PackageKitStatus {
    pub available: bool,
    pub backend_name: String,
    pub daemon_state: String,
    pub distro_id: String,
    pub locked: bool,
    pub network_state: u32,
    pub transactions: Vec<String>,
    pub version_major: u32,
    pub version_micro: u32,
    pub version_minor: u32,
}

/// Serializable snapshot of one installed package reported by PackageKit.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PackageKitPackage {
    pub info: u32,
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub arch: String,
    pub data: String,
    pub summary: String,
}

impl PackageKitPackage {
    pub(crate) fn from_signal(info: u32, package_id: &str, summary: &str) -> Option<Self> {
        let mut fields = package_id.splitn(4, ';');
        let name = fields.next()?.to_string();
        let version = fields.next()?.to_string();
        let arch = fields.next()?.to_string();
        let data = fields.next()?.to_string();

        data.starts_with("installed").then(|| Self {
            info,
            package_id: package_id.to_string(),
            name,
            version,
            arch,
            data,
            summary: summary.to_string(),
        })
    }
}
