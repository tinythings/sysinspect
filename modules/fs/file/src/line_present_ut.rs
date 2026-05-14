#[cfg(test)]
mod tests {
    use crate::line_present::{append_line, read_and_check};
    use std::io::Write as IoWrite;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TMP_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn tmp_path() -> PathBuf {
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("fsfile-lp-{}-{}", std::process::id(), n))
    }

    fn write_lines(path: &PathBuf, lines: &[&str]) {
        let mut f = std::fs::File::create(path).unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
    }

    #[test]
    fn line_already_present() {
        let p = tmp_path();
        write_lines(&p, &["foo=bar", "baz=qux"]);
        assert!(read_and_check(&p, "foo=bar").unwrap());
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn line_not_present() {
        let p = tmp_path();
        write_lines(&p, &["foo=bar"]);
        assert!(!read_and_check(&p, "baz=qux").unwrap());
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn append_adds_line_to_existing() {
        let p = tmp_path();
        write_lines(&p, &["first"]);
        append_line(&p, "second", true).unwrap();
        let contents = std::fs::read_to_string(&p).unwrap();
        assert_eq!(contents, "first\nsecond\n");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn append_creates_new_file() {
        let p = tmp_path();
        append_line(&p, "new-content", false).unwrap();
        let contents = std::fs::read_to_string(&p).unwrap();
        assert_eq!(contents, "new-content\n");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn append_preserves_trailing_newline() {
        let p = tmp_path();
        std::fs::write(&p, "first\n").unwrap();
        append_line(&p, "second", true).unwrap();
        let contents = std::fs::read_to_string(&p).unwrap();
        assert_eq!(contents, "first\nsecond\n");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn append_adds_missing_newline_before_append() {
        let p = tmp_path();
        std::fs::write(&p, "first").unwrap();
        append_line(&p, "second", true).unwrap();
        let contents = std::fs::read_to_string(&p).unwrap();
        assert_eq!(contents, "first\nsecond\n");
        std::fs::remove_file(&p).ok();
    }
}
