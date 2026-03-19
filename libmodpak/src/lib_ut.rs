#[cfg(test)]
mod tests {
    use crate::{SysInspectModPak, mpk::ModPakMetadata};
    use colored::control;
    use libsysinspect::cfg::mmconf::CFG_PROFILES_ROOT;
    use libsysinspect::{cfg::mmconf::MinionConfig, traits::effective_profiles};
    use std::collections::HashSet;
    use std::{fs, path::Path};

    /// Creates a minimal library tree under `src/lib`.
    ///
    /// Args:
    /// * `src` - Source root to populate.
    /// * `rel` - Relative file path below `src/lib`.
    ///
    /// Returns:
    /// * `()` after writing the file.
    fn write_library(src: &Path, rel: &str) {
        let dst = src.join("lib").join(rel);
        fs::create_dir_all(dst.parent().expect("library parent should exist")).expect("library parent should be created");
        fs::write(dst, format!("content for {rel}")).expect("library file should be written");
    }

    /// Creates a repository seeded with a few library files.
    ///
    /// Args:
    /// * None.
    ///
    /// Returns:
    /// * `(TempDir, SysInspectModPak)` with indexed library content.
    fn seeded_repo() -> (tempfile::TempDir, SysInspectModPak) {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let src = tempfile::tempdir().expect("src tempdir should be created");
        write_library(src.path(), "library/foo.py");
        write_library(src.path(), "library/bar.py");
        write_library(src.path(), "lua/baz.lua");

        let mut repo = SysInspectModPak::new(root.path().to_path_buf()).expect("repo should be created");
        repo.add_library(src.path().to_path_buf()).expect("library tree should be indexed");
        (root, repo)
    }

    /// Creates one indexed module artefact on disk.
    ///
    /// Args:
    /// * `repo` - Repository to seed.
    /// * `platform` - Indexed platform name.
    /// * `arch` - Indexed architecture name.
    /// * `name` - Module name.
    /// * `subpath` - Stored module subpath below script/<platform>/<arch>.
    ///
    /// Returns:
    /// * `()` after writing the file and indexing it.
    fn write_module(repo: &mut SysInspectModPak, platform: &str, arch: &str, name: &str, subpath: &str) {
        let dst = repo.root.join("script").join(platform).join(arch).join(subpath);
        fs::create_dir_all(dst.parent().expect("module parent should exist")).expect("module parent should be created");
        fs::write(&dst, format!("content for {name}")).expect("module file should be written");
        repo.idx.index_module(name, subpath, platform, arch, "demo module", false, "deadbeef", None, None).expect("module should be indexed");
        fs::write(repo.root.join("mod.index"), repo.idx.to_yaml().expect("index should serialize")).expect("index file should be written");
    }

    /// Creates a repository seeded with a few module files.
    ///
    /// Args:
    /// * None.
    ///
    /// Returns:
    /// * `(TempDir, SysInspectModPak)` with indexed module content.
    fn seeded_module_repo() -> (tempfile::TempDir, SysInspectModPak) {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let mut repo = SysInspectModPak::new(root.path().to_path_buf()).expect("repo should be created");
        write_module(&mut repo, "any", "noarch", "alpha.demo", "alpha/demo");
        write_module(&mut repo, "any", "noarch", "beta.demo", "beta/demo");
        write_module(&mut repo, "any", "noarch", "gamma.tool", "gamma/tool");
        (root, repo)
    }

    #[test]
    fn remove_library_supports_exact_names() {
        let (root, mut repo) = seeded_repo();

        repo.remove_library(vec!["lib/lua/baz.lua".to_string()]).expect("exact library removal should succeed");

        assert!(!root.path().join("lib/lib/lua/baz.lua").exists());
        assert!(root.path().join("lib/lib/library/foo.py").exists());
        assert!(root.path().join("lib/lib/library/bar.py").exists());
    }

    #[test]
    fn remove_library_supports_glob_patterns() {
        let (root, mut repo) = seeded_repo();

        repo.remove_library(vec!["library/*".to_string()]).expect("glob library removal should succeed");

        assert!(!root.path().join("lib/lib/library/foo.py").exists());
        assert!(!root.path().join("lib/lib/library/bar.py").exists());
        assert!(root.path().join("lib/lib/lua/baz.lua").exists());
    }

    #[test]
    fn remove_library_rejects_invalid_glob_patterns() {
        let (_, mut repo) = seeded_repo();

        assert!(repo.remove_library(vec!["library/[".to_string()]).is_err());
    }

    #[test]
    fn remove_module_supports_exact_names() {
        let (root, mut repo) = seeded_module_repo();

        repo.remove_module(vec!["gamma.tool"]).expect("exact module removal should succeed");

        assert!(!root.path().join("script/any/noarch/gamma/tool").exists());
        assert!(root.path().join("script/any/noarch/alpha/demo").exists());
        assert!(root.path().join("script/any/noarch/beta/demo").exists());
    }

    #[test]
    fn remove_module_supports_glob_patterns() {
        let (root, mut repo) = seeded_module_repo();

        repo.remove_module(vec!["*.demo"]).expect("glob module removal should succeed");

        assert!(!root.path().join("script/any/noarch/alpha/demo").exists());
        assert!(!root.path().join("script/any/noarch/beta/demo").exists());
        assert!(root.path().join("script/any/noarch/gamma/tool").exists());
    }

    #[test]
    fn remove_module_supports_wildcard_purge() {
        let (root, mut repo) = seeded_module_repo();

        repo.remove_module(vec!["*"]).expect("wildcard purge should succeed");

        assert!(!root.path().join("script/any/noarch/alpha/demo").exists());
        assert!(!root.path().join("script/any/noarch/beta/demo").exists());
        assert!(!root.path().join("script/any/noarch/gamma/tool").exists());
    }

    #[test]
    fn remove_module_rejects_invalid_glob_patterns() {
        let (_, mut repo) = seeded_module_repo();

        assert!(repo.remove_module(vec!["demo["]).is_err());
    }

    #[test]
    fn format_library_name_highlights_runtime_filenames() {
        control::set_override(true);

        let lua = SysInspectModPak::format_library_name("runtime/lua/reader.lua");
        let py3 = SysInspectModPak::format_library_name("runtime/python3/reader.py");
        let wasm = SysInspectModPak::format_library_name("runtime/wasm/hellodude.wasm");

        assert!(lua.contains("\u{1b}["));
        assert!(lua.contains("reader.lua"));
        assert!(py3.contains("\u{1b}["));
        assert!(py3.contains("reader.py"));
        assert!(wasm.contains("\u{1b}["));
        assert!(wasm.contains("hellodude.wasm"));
    }

    #[test]
    fn format_library_name_keeps_site_marker_bright_and_tail_dimmed() {
        control::set_override(true);

        let formatted = SysInspectModPak::format_library_name("runtime/python3/site-packages/mathx/__init__.py");

        assert!(formatted.contains("\u{1b}["));
        assert!(formatted.contains("site-packages/"));
        assert!(formatted.contains("mathx/__init__.py"));
    }

    #[test]
    fn add_library_indexes_wasm_payload_as_wasm_kind() {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let src = tempfile::tempdir().expect("src tempdir should be created");
        let payload = src.path().join("lib/runtime/wasm");
        fs::create_dir_all(&payload).expect("wasm payload dir should be created");
        fs::write(payload.join("demo.wasm"), b"\0asm\x01\0\0\0").expect("wasm payload should be written");

        let mut repo = SysInspectModPak::new(root.path().to_path_buf()).expect("repo should be created");
        repo.add_library(src.path().to_path_buf()).expect("library tree should be indexed");

        let library = repo.idx.library();
        let entry = library.get("lib/runtime/wasm/demo.wasm").expect("wasm library entry should exist");
        assert_eq!(entry.kind(), "wasm");
    }

    #[test]
    fn add_library_indexes_elf_payload_as_binary_kind() {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let src = tempfile::tempdir().expect("src tempdir should be created");
        let payload = src.path().join("lib/runtime/native");
        fs::create_dir_all(&payload).expect("binary payload dir should be created");
        fs::copy("/bin/sh", payload.join("demo")).expect("binary payload should be copied");

        let mut repo = SysInspectModPak::new(root.path().to_path_buf()).expect("repo should be created");
        repo.add_library(src.path().to_path_buf()).expect("library tree should be indexed");

        let library = repo.idx.library();
        let entry = library.get("lib/runtime/native/demo").expect("binary library entry should exist");
        assert_eq!(entry.kind(), "binary");
    }

    #[test]
    fn add_module_installs_binary_under_namespace_path_not_source_filename() {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let src = tempfile::tempdir().expect("src tempdir should be created");
        let binary = src.path().join("lua-runtime");
        fs::copy("/bin/sh", &binary).expect("test binary should be copied");

        let mut repo = SysInspectModPak::new(root.path().to_path_buf()).expect("repo should be created");
        let meta = ModPakMetadata::new_for_test(binary, "runtime.lua");
        repo.add_module(meta).expect("module should be added");

        let idx = repo.idx.all_modules(None, Some(vec!["runtime.lua"]));
        let mut found = false;
        for archset in idx.values() {
            for modules in archset.values() {
                if let Some(attrs) = modules.get("runtime.lua") {
                    assert_eq!(attrs.subpath(), "runtime/lua");
                    found = true;
                }
            }
        }
        assert!(found, "runtime.lua should be indexed");
    }

    #[test]
    fn profile_crud_updates_index_and_profile_file() {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let repo = SysInspectModPak::new(root.path().join("repo")).expect("repo should be created");

        repo.new_profile("toto").expect("profile should be created");
        repo.add_profile_matches("toto", vec!["runtime.lua".to_string(), "net.*".to_string()], false).expect("module selectors should be added");
        repo.add_profile_matches("toto", vec!["runtime/lua/*.lua".to_string()], true).expect("library selectors should be added");

        assert_eq!(repo.list_profiles(None).expect("profiles should list"), vec!["toto".to_string()]);
        assert!(repo.list_profile_matches(Some("toto"), false).expect("profile modules should list").contains(&"toto: runtime.lua".to_string()));
        assert!(repo.list_profile_matches(Some("toto"), false).expect("profile modules should list").contains(&"toto: net.*".to_string()));
        assert!(
            repo.list_profile_matches(Some("toto"), true).expect("profile libraries should list").contains(&"toto: runtime/lua/*.lua".to_string())
        );

        repo.remove_profile_matches("toto", vec!["net.*".to_string()], false).expect("module selector should be removed");
        assert!(!repo.list_profile_matches(Some("toto"), false).expect("profile modules should list").contains(&"toto: net.*".to_string()));

        repo.delete_profile("toto").expect("profile should be deleted");
        assert!(repo.list_profiles(None).expect("profiles should list").is_empty());
    }

    #[test]
    fn profile_create_and_delete_validate_existence() {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let repo = SysInspectModPak::new(root.path().join("repo")).expect("repo should be created");

        assert!(repo.delete_profile("missing").is_err());
        repo.new_profile("toto").expect("profile should be created");
        assert!(repo.new_profile("toto").is_err());
    }

    #[test]
    fn new_profiles_use_lowercase_filenames_without_changing_profile_name() {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let repo = SysInspectModPak::new(root.path().join("repo")).expect("repo should be created");

        repo.new_profile("Toto").expect("profile should be created");

        let idx = repo.get_profiles_index().expect("profiles index should load");
        let profile = repo.get_profile("Toto").expect("profile should load");
        assert_eq!(idx.get("Toto").expect("profile ref should exist").file(), &std::path::PathBuf::from("toto.profile"));
        assert_eq!(profile.name(), "Toto");
    }

    #[test]
    fn existing_profile_keeps_arbitrary_indexed_filename() {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let repo = SysInspectModPak::new(root.path().join("repo")).expect("repo should be created");
        let profiles_root = root.path().join(CFG_PROFILES_ROOT);
        fs::write(profiles_root.join("totobullshit.profile"), "name: Toto\nmodules:\n  - runtime.lua\n").expect("profile file should be written");
        fs::write(root.path().join("profiles.index"), "profiles:\n  Toto:\n    file: totobullshit.profile\n    checksum: deadbeef\n")
            .expect("profiles index should be written");

        repo.add_profile_matches("Toto", vec!["net.*".to_string()], false).expect("profile should be updated");

        let idx = repo.get_profiles_index().expect("profiles index should load");
        let profile = repo.get_profile("Toto").expect("profile should load");
        assert_eq!(idx.get("Toto").expect("profile ref should exist").file(), &std::path::PathBuf::from("totobullshit.profile"));
        assert_eq!(profile.name(), "Toto");
        assert!(profile.modules().contains(&"runtime.lua".to_string()));
        assert!(profile.modules().contains(&"net.*".to_string()));
    }

    #[test]
    fn profiles_index_rejects_parent_dir_traversal() {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let repo = SysInspectModPak::new(root.path().join("repo")).expect("repo should be created");
        fs::write(root.path().join("profiles.index"), "profiles:\n  Toto:\n    file: ../escape.profile\n    checksum: deadbeef\n")
            .expect("profiles index should be written");

        assert!(repo.get_profiles_index().is_err());
    }

    #[test]
    fn new_profile_rejects_traversing_name() {
        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let repo = SysInspectModPak::new(root.path().join("repo")).expect("repo should be created");

        assert!(repo.new_profile("../escape").is_err());
    }

    #[test]
    fn show_profile_renders_modules_first_and_libraries_after() {
        control::set_override(true);

        let root = tempfile::tempdir().expect("repo tempdir should be created");
        let src = tempfile::tempdir().expect("src tempdir should be created");
        let repo_root = root.path().join("repo");
        let mut repo = SysInspectModPak::new(repo_root.clone()).expect("repo should be created");
        write_library(src.path(), "runtime/lua/reader.lua");
        repo.add_library(src.path().to_path_buf()).expect("library tree should be indexed");
        write_module(&mut repo, "linux", "x86_64", "runtime.lua", "runtime/lua");
        write_module(&mut repo, "netbsd", "noarch", "runtime.lua", "runtime/lua");
        repo.new_profile("toto").expect("profile should be created");
        repo.add_profile_matches("toto", vec!["runtime.lua".to_string()], false).expect("module selector should be added");
        repo.add_profile_matches("toto", vec!["lib/runtime/lua/*.lua".to_string()], true).expect("library selector should be added");

        let rendered = repo.show_profile("toto").expect("profile should render");
        let module_pos = rendered.find("runtime.lua").expect("module row should exist");
        let library_pos = rendered.find("reader.lua").expect("library row should exist");

        assert!(rendered.contains("Linux, NetBSD") || rendered.contains("NetBSD, Linux"));
        assert!(rendered.contains("x86_64, noarch") || rendered.contains("noarch, x86_64"));
        assert!(module_pos < library_pos, "modules should be rendered before libraries");
    }

    #[test]
    fn effective_profile_names_fallback_to_default_and_accept_array() {
        let root = tempfile::tempdir().expect("root tempdir should be created");
        let share = tempfile::tempdir().expect("share tempdir should be created");
        let mut cfg = MinionConfig::default();
        cfg.set_root_dir(root.path().to_str().expect("root path should be valid"));
        cfg.set_sharelib_path(share.path().to_str().expect("share path should be valid"));
        fs::create_dir_all(cfg.traits_dir()).expect("traits dir should be created");

        assert_eq!(effective_profiles(&cfg), vec!["default".to_string()]);

        fs::write(cfg.traits_dir().join("master.cfg"), "minion.profile:\n  - Toto\n  - Foo\n  - Toto\n").expect("master traits should be written");
        let names = effective_profiles(&cfg).into_iter().collect::<HashSet<_>>();
        assert_eq!(names, HashSet::from(["Toto".to_string(), "Foo".to_string()]));
    }
}
