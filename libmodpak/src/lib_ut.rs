#[cfg(test)]
mod tests {
    use crate::{SysInspectModPak, mpk::ModPakMetadata};
    use colored::control;
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
}
