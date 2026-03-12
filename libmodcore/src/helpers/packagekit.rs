use libcommon::SysinspectError;
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::{OwnedObjectPath, OwnedValue},
};

const PK_DEST: &str = "org.freedesktop.PackageKit";
const PK_PATH: &str = "/org/freedesktop/PackageKit";
const PK_IFACE: &str = "org.freedesktop.PackageKit";
const PK_TX_IFACE: &str = "org.freedesktop.PackageKit.Transaction";

/// Shared PackageKit helper API for runtime hosts.
pub struct RuntimePackageKit;

impl RuntimePackageKit {
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
    /// * `Err(SysinspectError)` if PackageKit is unavailable or D-Bus access fails.
    pub fn status() -> Result<PackageKitStatus, SysinspectError> {
        let proxy = Self::root_proxy()?;
        Ok(PackageKitStatus {
            available: true,
            backend_name: proxy.get_property("BackendName").map_err(Self::dbus_err)?,
            distro_id: proxy.get_property("DistroId").map_err(Self::dbus_err)?,
            daemon_state: proxy.call("GetDaemonState", &()).map_err(Self::dbus_err)?,
            locked: proxy.get_property("Locked").map_err(Self::dbus_err)?,
            network_state: proxy.get_property("NetworkState").map_err(Self::dbus_err)?,
            version_major: proxy.get_property("VersionMajor").map_err(Self::dbus_err)?,
            version_micro: proxy.get_property("VersionMicro").map_err(Self::dbus_err)?,
            version_minor: proxy.get_property("VersionMinor").map_err(Self::dbus_err)?,
            transactions: proxy.call("GetTransactionList", &()).map_err(Self::dbus_err)?,
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
    /// * `Err(SysinspectError)` if D-Bus access or JSON conversion fails.
    pub fn history(names: Vec<String>, count: u32) -> Result<serde_json::Value, SysinspectError> {
        let proxy = Self::root_proxy()?;
        let data =
            proxy.call::<_, _, BTreeMap<String, Vec<BTreeMap<String, OwnedValue>>>>("GetPackageHistory", &(names, count)).map_err(Self::dbus_err)?;
        serde_json::to_value(data).map_err(SysinspectError::from)
    }

    /// Returns a snapshot of installed packages currently known to PackageKit.
    ///
    /// Returns:
    /// * `Ok(serde_json::Value)` containing an array of installed package objects.
    /// * `Err(SysinspectError)` if D-Bus access or transaction processing fails.
    pub fn packages() -> Result<serde_json::Value, SysinspectError> {
        serde_json::to_value(Self::collect_installed_packages()?).map_err(SysinspectError::from)
    }

    /// Installs packages by name through PackageKit.
    ///
    /// Arguments:
    /// * `names` - Package names to resolve and install.
    ///
    /// Returns:
    /// * `Ok(serde_json::Value)` describing requested names and resolved package ids.
    /// * `Err(SysinspectError)` if resolution or installation fails.
    pub fn install(names: Vec<String>) -> Result<serde_json::Value, SysinspectError> {
        if names.is_empty() {
            return Err(SysinspectError::ModuleError("PackageKit install requires at least one package name".to_string()));
        }

        let package_ids = Self::resolve_package_ids(&names)?;
        let changed = Self::install_package_ids(&package_ids)?;

        Ok(json!({
            "requested": names,
            "package_ids": package_ids,
            "changed": changed,
        }))
    }

    fn root_proxy() -> Result<Proxy<'static>, SysinspectError> {
        Connection::system().map_err(Self::dbus_err).and_then(|conn| Proxy::new(&conn, PK_DEST, PK_PATH, PK_IFACE).map_err(Self::dbus_err))
    }

    fn collect_installed_packages() -> Result<Vec<PackageKitPackage>, SysinspectError> {
        let conn = Connection::system().map_err(Self::dbus_err)?;
        let root = Proxy::new(&conn, PK_DEST, PK_PATH, PK_IFACE).map_err(Self::dbus_err)?;
        let tx_path = root.call::<_, _, OwnedObjectPath>("CreateTransaction", &()).map_err(Self::dbus_err)?;
        let tx = Proxy::new(&conn, PK_DEST, tx_path.as_str(), PK_TX_IFACE).map_err(Self::dbus_err)?;
        let mut signals = tx.receive_all_signals().map_err(Self::dbus_err)?;
        let mut packages = Vec::new();

        tx.call::<_, _, ()>("GetPackages", &(0u64,)).map_err(Self::dbus_err)?;

        for msg in &mut signals {
            match msg.header().member().map(|m| m.as_str()) {
                Some("Package") => {
                    let (info, package_id, summary) = msg.body().deserialize::<(u32, String, String)>().map_err(Self::dbus_err)?;
                    if let Some(pkg) = PackageKitPackage::from_signal(info, &package_id, &summary) {
                        packages.push(pkg);
                    }
                }
                Some("ErrorCode") => {
                    let (code, details) = msg.body().deserialize::<(u32, String)>().map_err(Self::dbus_err)?;
                    return Err(SysinspectError::ModuleError(format!("PackageKit transaction failed with error code {code}: {details}")));
                }
                Some("Finished") => break,
                _ => (),
            }
        }

        Ok(packages)
    }

    fn resolve_package_ids(names: &[String]) -> Result<Vec<String>, SysinspectError> {
        let conn = Connection::system().map_err(Self::dbus_err)?;
        let root = Proxy::new(&conn, PK_DEST, PK_PATH, PK_IFACE).map_err(Self::dbus_err)?;
        let tx_path = root.call::<_, _, OwnedObjectPath>("CreateTransaction", &()).map_err(Self::dbus_err)?;
        let tx = Proxy::new(&conn, PK_DEST, tx_path.as_str(), PK_TX_IFACE).map_err(Self::dbus_err)?;
        let mut signals = tx.receive_all_signals().map_err(Self::dbus_err)?;
        let mut package_ids = Vec::new();

        tx.call::<_, _, ()>("Resolve", &(0u64, names.to_vec())).map_err(Self::dbus_err)?;

        for msg in &mut signals {
            match msg.header().member().map(|m| m.as_str()) {
                Some("Package") => {
                    let (_, package_id, _) = msg.body().deserialize::<(u32, String, String)>().map_err(Self::dbus_err)?;
                    if Self::is_available_package_id(&package_id) {
                        package_ids.push(package_id);
                    }
                }
                Some("ErrorCode") => {
                    let (code, details) = msg.body().deserialize::<(u32, String)>().map_err(Self::dbus_err)?;
                    return Err(SysinspectError::ModuleError(format!("PackageKit resolve failed with error code {code}: {details}")));
                }
                Some("Finished") => break,
                _ => (),
            }
        }

        package_ids.sort();
        package_ids.dedup();

        if package_ids.is_empty() {
            return Err(SysinspectError::ModuleError(format!("PackageKit could not resolve installable package ids for {}", names.join(", "))));
        }

        Ok(package_ids)
    }

    fn install_package_ids(package_ids: &[String]) -> Result<Vec<String>, SysinspectError> {
        let conn = Connection::system().map_err(Self::dbus_err)?;
        let root = Proxy::new(&conn, PK_DEST, PK_PATH, PK_IFACE).map_err(Self::dbus_err)?;
        let tx_path = root.call::<_, _, OwnedObjectPath>("CreateTransaction", &()).map_err(Self::dbus_err)?;
        let tx = Proxy::new(&conn, PK_DEST, tx_path.as_str(), PK_TX_IFACE).map_err(Self::dbus_err)?;
        let mut signals = tx.receive_all_signals().map_err(Self::dbus_err)?;
        let mut changed = Vec::new();

        tx.call::<_, _, ()>("InstallPackages", &(0u64, package_ids.to_vec())).map_err(Self::dbus_err)?;

        for msg in &mut signals {
            match msg.header().member().map(|m| m.as_str()) {
                Some("Package") => {
                    let (_, package_id, _) = msg.body().deserialize::<(u32, String, String)>().map_err(Self::dbus_err)?;
                    changed.push(package_id);
                }
                Some("ErrorCode") => {
                    let (code, details) = msg.body().deserialize::<(u32, String)>().map_err(Self::dbus_err)?;
                    return Err(SysinspectError::ModuleError(format!("PackageKit install failed with error code {code}: {details}")));
                }
                Some("Finished") => break,
                _ => (),
            }
        }

        changed.sort();
        changed.dedup();
        if changed.is_empty() {
            changed.extend(package_ids.iter().cloned());
        }

        Ok(changed)
    }

    pub(crate) fn is_available_package_id(package_id: &str) -> bool {
        package_id.splitn(4, ';').nth(3).is_some_and(|data| !data.starts_with("installed"))
    }

    fn dbus_err(err: zbus::Error) -> SysinspectError {
        SysinspectError::ModuleError(format!("PackageKit D-Bus error: {err}"))
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
    pub fn from_signal(info: u32, package_id: &str, summary: &str) -> Option<Self> {
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
