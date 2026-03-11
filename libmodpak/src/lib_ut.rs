#[cfg(test)]
mod tests {
    use crate::SysInspectModPak;
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
        write_library(src.path(), "ansible/foo.py");
        write_library(src.path(), "ansible/bar.py");
        write_library(src.path(), "lua/baz.lua");

        let mut repo = SysInspectModPak::new(root.path().to_path_buf()).expect("repo should be created");
        repo.add_library(src.path().to_path_buf()).expect("library tree should be indexed");
        (root, repo)
    }

    #[test]
    fn remove_library_supports_exact_names() {
        let (root, mut repo) = seeded_repo();

        repo.remove_library(vec!["lib/lua/baz.lua".to_string()]).expect("exact library removal should succeed");

        assert!(!root.path().join("lib/lib/lua/baz.lua").exists());
        assert!(root.path().join("lib/lib/ansible/foo.py").exists());
        assert!(root.path().join("lib/lib/ansible/bar.py").exists());
    }

    #[test]
    fn remove_library_supports_glob_patterns() {
        let (root, mut repo) = seeded_repo();

        repo.remove_library(vec!["ansible/*".to_string()]).expect("glob library removal should succeed");

        assert!(!root.path().join("lib/lib/ansible/foo.py").exists());
        assert!(!root.path().join("lib/lib/ansible/bar.py").exists());
        assert!(root.path().join("lib/lib/lua/baz.lua").exists());
    }

    #[test]
    fn remove_library_rejects_invalid_glob_patterns() {
        let (_, mut repo) = seeded_repo();

        assert!(repo.remove_library(vec!["ansible/[".to_string()]).is_err());
    }
}
