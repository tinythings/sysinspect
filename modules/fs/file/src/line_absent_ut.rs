#[cfg(test)]
mod tests {
    use crate::line_absent::remove_matching_lines;
    use std::io::Write as IoWrite;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TMP_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn tmp_path() -> PathBuf {
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("fsfile-la-{}-{}", std::process::id(), n))
    }

    fn write_lines(path: &PathBuf, lines: &[&str]) {
        let mut f = std::fs::File::create(path).unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
    }

    #[test]
    fn removes_single_matching_line() {
        let p = tmp_path();
        write_lines(&p, &["keep-me", "delete-me", "also-keep"]);
        let (count, output) = remove_matching_lines(&p, "delete-me").unwrap();
        assert_eq!(count, 1);
        assert_eq!(output, "keep-me\nalso-keep\n");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn removes_all_duplicate_matches() {
        let p = tmp_path();
        write_lines(&p, &["dup", "unique", "dup", "dup"]);
        let (count, output) = remove_matching_lines(&p, "dup").unwrap();
        assert_eq!(count, 3);
        assert_eq!(output, "unique\n");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn returns_zero_when_no_match() {
        let p = tmp_path();
        write_lines(&p, &["a", "b", "c"]);
        let (count, _) = remove_matching_lines(&p, "nonexistent").unwrap();
        assert_eq!(count, 0);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn handles_empty_file() {
        let p = tmp_path();
        write_lines(&p, &[]);
        let (count, _) = remove_matching_lines(&p, "anything").unwrap();
        assert_eq!(count, 0);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn preserves_trailing_newline_after_removal() {
        let p = tmp_path();
        write_lines(&p, &["one", "two", "three"]);
        let (count, output) = remove_matching_lines(&p, "two").unwrap();
        assert_eq!(count, 1);
        assert!(output.ends_with('\n'));
        std::fs::remove_file(&p).ok();
    }
}
