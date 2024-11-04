mod systraits;

use once_cell::sync::Lazy;
use std::sync::Mutex;
use systraits::SystemTraits;

/// Standard Traits
pub static SYS_ID: &str = "system.id";
pub static SYS_OS_KERNEL: &str = "system.kernel";
pub static SYS_OS_VERSION: &str = "system.os.version";
pub static SYS_OS_NAME: &str = "system.os.name";
pub static SYS_OS_DISTRO: &str = "system.os.distribution";

pub static SYS_NET_HOSTNAME: &str = "system.hostname";
pub static HW_MEM: &str = "hardware.memory";
pub static HW_SWAP: &str = "hardware.swap";
pub static HW_CPU_TOTAL: &str = "hardware.cpu.total";
pub static HW_CPU_BRAND: &str = "hardware.cpu.brand";
pub static HW_CPU_FREQ: &str = "hardware.cpu.frequency";
pub static HW_CPU_VENDOR: &str = "hardware.cpu.vendor";
pub static HW_CPU_CORES: &str = "hardware.cpu.cores";

/*
Traits are system properties and attributes on which a minion is running.

P.S. These are not Rust traits. :-)
 */

/// System traits instance
static TRAITS: Lazy<Mutex<SystemTraits>> = Lazy::new(|| Mutex::new(SystemTraits::new()));

/// Returns a copy of initialised traits.
pub fn get_traits() -> SystemTraits {
    let traits = &TRAITS;
    if let Ok(traits) = traits.lock() {
        return traits.to_owned();
    }

    SystemTraits::default()
}
