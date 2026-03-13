use crate::mpk::{ModPakMetadata, ModPakProfile, ModPakProfilesIndex, ModPakRepoIndex};
use indexmap::IndexSet;
use std::path::PathBuf;

#[test]
fn runtime_dispatcher_names_are_reserved() {
    let meta = ModPakMetadata::new_for_test(PathBuf::from("/tmp/lua-runtime"), "runtime.lua");
    assert!(meta.validate_namespace().is_ok());

    let meta = ModPakMetadata::new_for_test(PathBuf::from("/tmp/not-a-runtime"), "runtime.lua");
    assert!(meta.validate_namespace().is_err());

    let meta = ModPakMetadata::new_for_test(PathBuf::from("/tmp/custom-module"), "lua.reader");
    assert!(meta.validate_namespace().is_err());
}

#[test]
fn profiles_index_and_profile_roundtrip() {
    let mut index = ModPakProfilesIndex::new();
    index.insert("default", PathBuf::from("default.profile"), "deadbeef");
    let index = ModPakProfilesIndex::from_yaml(&index.to_yaml().expect("profiles index should serialize")).expect("profiles index should deserialize");
    let profile =
        ModPakProfile::from_yaml("name: default\nmodules:\n  - runtime.lua\nlibraries:\n  - runtime/lua/reader.lua\n").expect("profile should deserialize");

    assert_eq!(index.get("default").expect("default profile should exist").file(), &PathBuf::from("default.profile"));
    assert_eq!(profile.name(), "default");
    assert_eq!(profile.modules(), &["runtime.lua".to_string()]);
    assert_eq!(profile.libraries(), &["runtime/lua/reader.lua".to_string()]);
}

#[test]
fn profile_merge_and_repo_filter_dedup_exact_matches() {
    let mut modules = IndexSet::new();
    let mut libraries = IndexSet::new();
    ModPakProfile::from_yaml("name: default\nmodules:\n  - runtime.lua\n  - runtime.lua\nlibraries:\n  - runtime/lua/reader.lua\n")
        .expect("profile should deserialize")
        .merge_into(&mut modules, &mut libraries);

    let mut repo = ModPakRepoIndex::from_yaml(
        r#"
platform: {}
library:
  runtime/lua/reader.lua:
    file: runtime/lua/reader.lua
    checksum: beadfeed
    kind: lua
  runtime/py3/reader.py:
    file: runtime/py3/reader.py
    checksum: facefeed
    kind: python
"#,
    )
    .expect("repo index should deserialize");
    repo.index_module("runtime.lua", "runtime/lua", "any", "noarch", "lua runtime", false, "deadbeef", None, None)
        .expect("runtime module should index");
    repo.index_module("net.ping", "net/ping", "any", "noarch", "ping module", false, "cafebabe", None, None)
        .expect("ping module should index");

    let filtered = repo.retain_profiles(&modules, &libraries);
    let modules = filtered.modules();
    let libraries = filtered.library();

    assert!(modules.contains_key("runtime.lua"));
    assert!(!modules.contains_key("net.ping"));
    assert!(libraries.contains_key("runtime/lua/reader.lua"));
    assert!(!libraries.contains_key("runtime/py3/reader.py"));
}
