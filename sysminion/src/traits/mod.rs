mod systraits;

use once_cell::sync::Lazy;
use std::sync::Mutex;
use systraits::SystemTraits;

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
