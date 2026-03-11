use crate::layout::{get_script_root, get_site_root};
use std::path::Path;

#[test]
fn script_and_site_roots_are_under_sharelib() {
    assert_eq!(get_script_root(Path::new("/opt/sysinspect")), Path::new("/opt/sysinspect/lib/sensors/lua54"));
    assert_eq!(get_site_root(Path::new("/opt/sysinspect")), Path::new("/opt/sysinspect/lib/sensors/lua54/site-lua"));
}
