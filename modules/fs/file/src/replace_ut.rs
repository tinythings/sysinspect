#[cfg(test)]
mod tests {
    use crate::replace::do_replace;
    use std::io::Write as IoWrite;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TMP_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn tmp_path() -> PathBuf {
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("fsfile-rp-{}-{}", std::process::id(), n))
    }

    fn write_lines(path: &PathBuf, lines: &[&str]) {
        let mut f = std::fs::File::create(path).unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
    }

    #[test]
    fn replaces_single_occurrence() {
        let p = tmp_path();
        write_lines(&p, &["hello world", "goodbye"]);
        let (count, output) = do_replace(&p, "hello", "hi").unwrap();
        assert_eq!(count, 1);
        assert_eq!(output, "hi world\ngoodbye\n");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn replaces_multiple_occurrences() {
        let p = tmp_path();
        write_lines(&p, &["foo bar foo", "foo baz"]);
        let (count, output) = do_replace(&p, "foo", "qux").unwrap();
        assert_eq!(count, 2);
        assert_eq!(output, "qux bar qux\nqux baz\n");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn returns_zero_when_no_match() {
        let p = tmp_path();
        write_lines(&p, &["one", "two"]);
        let (count, _) = do_replace(&p, "nonexistent", "replacement").unwrap();
        assert_eq!(count, 0);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn preserves_trailing_newline() {
        let p = tmp_path();
        write_lines(&p, &["a", "b"]);
        let (_, output) = do_replace(&p, "a", "x").unwrap();
        assert!(output.ends_with('\n'));
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn preserves_no_trailing_newline() {
        let p = tmp_path();
        std::fs::write(&p, "no-newline").unwrap();
        let (_, output) = do_replace(&p, "no", "yes").unwrap();
        assert_eq!(output, "yes-newline");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn handles_empty_file() {
        let p = tmp_path();
        write_lines(&p, &[]);
        let (count, _) = do_replace(&p, "anything", "nothing").unwrap();
        assert_eq!(count, 0);
        std::fs::remove_file(&p).ok();
    }
}
