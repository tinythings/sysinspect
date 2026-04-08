use crate::local_marker::LocalMarker;

#[test]
fn local_marker_roundtrip_yaml() {
    let marker = LocalMarker::hopstart("/opt/sysinspect");

    assert_eq!(LocalMarker::from_yaml(&marker.to_yaml().unwrap()).unwrap(), marker);
}

#[test]
fn local_marker_rejects_relative_root() {
    assert!(LocalMarker::from_yaml("root: sysinspect\ninit: hopstart\n").is_err());
}
