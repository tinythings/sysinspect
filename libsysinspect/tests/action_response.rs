#[cfg(test)]
mod tests {
    use libsysinspect::intp::actproc::response::{ActionModResponse, ActionResponse, ConstraintResponse};
    use serde_json::json;

    fn mk_ar(aid: &str, eid: &str, sid: &str, ret: i32) -> ActionResponse {
        ActionResponse::new(eid.to_string(), aid.to_string(), sid.to_string(), ActionModResponse::with_retcode(ret), ConstraintResponse::default())
    }

    // -------------------------
    // from_sensor() tests
    // -------------------------

    #[test]
    fn from_sensor_parses_4_parts_and_sets_fields() {
        let v = json!({
            "eid": "tmp-watch|fsnotify|deleted@/tmp/x|0",
            "sensor": "tmp-watch",
            "listener": "fsnotify",
            "data": {"kind":"deleted","path":"/tmp/x"}
        });

        let ar = ActionResponse::from_sensor(v.clone());

        assert_eq!(ar.aid(), "tmp-watch");
        assert_eq!(ar.eid(), "fsnotify");
        assert_eq!(ar.sid(), "deleted@/tmp/x");
        // current behavior: retcode forced to 0 even if eid contains "|123"
        assert_eq!(ar.response.retcode(), 0);

        // payload stored as data
        assert_eq!(ar.response.data().unwrap(), v);
    }

    #[test]
    fn from_sensor_fallback_when_not_4_parts() {
        let v = json!({"eid":"nonsense", "data":{"x":1}});
        let ar = ActionResponse::from_sensor(v.clone());

        // fallback: only ar.eid set
        assert_eq!(ar.eid(), "nonsense");
        assert_eq!(ar.aid(), "");
        // sid() returns "$" when empty
        assert_eq!(ar.sid(), "$");
        assert_eq!(ar.response.retcode(), 0);
        assert_eq!(ar.response.data().unwrap(), v);
    }

    #[test]
    fn from_sensor_default_eid_when_missing() {
        let v = json!({"data":{"x":1}});
        let ar = ActionResponse::from_sensor(v.clone());
        assert_eq!(ar.aid(), "$");
        assert_eq!(ar.eid(), "$");
        assert_eq!(ar.sid(), "$");
        assert_eq!(ar.response.retcode(), 0);
        assert_eq!(ar.response.data().unwrap(), v);
    }

    // -------------------------
    // glob_match() tests
    // -------------------------

    #[test]
    fn glob_match_basic_dollar_is_wildcard() {
        assert!(ActionResponse::glob_match("$", ""));
        assert!(ActionResponse::glob_match("$", "anything"));
        assert!(ActionResponse::glob_match("/tmp/$", "/tmp/x"));
        assert!(ActionResponse::glob_match("/tmp/$", "/tmp/foo/bar"));
        assert!(!ActionResponse::glob_match("/tmp/$", "/etc/x"));
    }

    #[test]
    fn glob_match_middle_wildcard() {
        assert!(ActionResponse::glob_match("/tmp/$/x", "/tmp/a/x"));
        assert!(ActionResponse::glob_match("/tmp/$/x", "/tmp/a/b/c/x"));
        assert!(!ActionResponse::glob_match("/tmp/$/x", "/tmp/a/y"));
    }

    #[test]
    fn glob_match_escapes_regex_metachars() {
        // '.' should be literal dot, not any-char
        assert!(ActionResponse::glob_match("a.b", "a.b"));
        assert!(!ActionResponse::glob_match("a.b", "acb"));

        // '+' literal
        assert!(ActionResponse::glob_match("a+b", "a+b"));
        assert!(!ActionResponse::glob_match("a+b", "aaab"));

        // brackets literal
        assert!(ActionResponse::glob_match("[x]", "[x]"));
        assert!(!ActionResponse::glob_match("[x]", "x"));
    }

    // -------------------------
    // sid_matches() tests
    // -------------------------

    #[test]
    fn sid_matches_dollar_matches_anything() {
        assert!(ActionResponse::sid_matches("whatever", "$"));
        assert!(ActionResponse::sid_matches("deleted@/tmp/x", "$"));
        assert!(ActionResponse::sid_matches("", "$"));
    }

    #[test]
    fn sid_matches_exact_without_at() {
        assert!(ActionResponse::sid_matches("deleted:/tmp/x", "deleted:/tmp/x"));
        assert!(!ActionResponse::sid_matches("deleted:/tmp/x", "deleted:/tmp/y"));
    }

    #[test]
    fn sid_matches_pattern_with_at_requires_value_with_at() {
        assert!(!ActionResponse::sid_matches("deleted:/tmp/x", "deleted@/tmp/$"));
        assert!(!ActionResponse::sid_matches("deleted", "deleted@$"));
    }

    #[test]
    fn sid_matches_kind_must_match() {
        assert!(!ActionResponse::sid_matches("created@/tmp/x", "deleted@/tmp/$"));
        assert!(ActionResponse::sid_matches("deleted@/tmp/x", "deleted@/tmp/$"));
    }

    #[test]
    fn sid_matches_kind_at_dollar_means_any_detail() {
        assert!(ActionResponse::sid_matches("deleted@/tmp/x", "deleted@$"));
        assert!(ActionResponse::sid_matches("deleted@/etc/passwd", "deleted@$"));
        assert!(!ActionResponse::sid_matches("created@/tmp/x", "deleted@$"));
    }

    #[test]
    fn sid_matches_detail_glob_with_dollar() {
        assert!(ActionResponse::sid_matches("deleted@/tmp/x", "deleted@/tmp/$"));
        assert!(ActionResponse::sid_matches("deleted@/tmp/foo/bar", "deleted@/tmp/$"));
        assert!(!ActionResponse::sid_matches("deleted@/etc/x", "deleted@/tmp/$"));
    }

    // -------------------------
    // match_eid() matrix tests
    // -------------------------

    #[test]
    fn match_eid_exact_match_all_parts() {
        let ar = mk_ar("tmp-watch", "fsnotify", "deleted@/tmp/x", 0);
        assert!(ar.match_eid("tmp-watch|fsnotify|deleted@/tmp/x|0"));
        assert!(!ar.match_eid("tmp-watch|fsnotify|deleted@/tmp/y|0"));
        assert!(!ar.match_eid("tmp-watch|other|deleted@/tmp/x|0"));
        assert!(!ar.match_eid("other|fsnotify|deleted@/tmp/x|0"));
    }

    #[test]
    fn match_eid_wildcards_dollar_in_any_field() {
        let ar = mk_ar("A", "B", "C@/tmp/x", 0);

        assert!(ar.match_eid("$|$|$|$"));
        assert!(ar.match_eid("$|B|C@/tmp/x|0"));
        assert!(ar.match_eid("A|$|C@/tmp/x|0"));
        assert!(ar.match_eid("A|B|$|0"));
        assert!(ar.match_eid("A|B|C@/tmp/$|0")); // sid glob
    }

    #[test]
    fn match_eid_retcode_dollar_matches_any_retcode() {
        let ar0 = mk_ar("A", "B", "C@/x", 0);
        let ar5 = mk_ar("A", "B", "C@/x", 5);

        assert!(ar0.match_eid("A|B|C@/x|$"));
        assert!(ar5.match_eid("A|B|C@/x|$"));
    }

    #[test]
    fn match_eid_retcode_exact_numeric() {
        let ar0 = mk_ar("A", "B", "C@/x", 0);
        let ar5 = mk_ar("A", "B", "C@/x", 5);

        assert!(ar0.match_eid("A|B|C@/x|0"));
        assert!(!ar0.match_eid("A|B|C@/x|5"));

        assert!(ar5.match_eid("A|B|C@/x|5"));
        assert!(!ar5.match_eid("A|B|C@/x|0"));
    }

    #[test]
    fn match_eid_retcode_e_means_error_only() {
        let ok = mk_ar("A", "B", "C@/x", 0);
        let err = mk_ar("A", "B", "C@/x", 2);

        assert!(!ok.match_eid("A|B|C@/x|E"));
        assert!(err.match_eid("A|B|C@/x|E"));
    }

    #[test]
    fn match_eid_rejects_bad_format() {
        let ar = mk_ar("A", "B", "C@/x", 0);

        assert!(!ar.match_eid("A|B|C@/x")); // 3 parts
        assert!(!ar.match_eid("A|B|C@/x|0|junk")); // 5 parts
        assert!(!ar.match_eid("")); // 0 parts
    }

    // "Diagonal" matrix: many cases in a table-like loop
    #[test]
    fn match_eid_matrix_diagonal() {
        let cases = vec![
            // (ar_aid, ar_eid, ar_sid, ar_ret, pattern, expected)
            ("a", "b", "k@/tmp/x", 0, "a|b|k@/tmp/x|0", true),
            ("a", "b", "k@/tmp/x", 0, "a|b|k@/tmp/$|0", true),
            ("a", "b", "k@/tmp/x", 0, "a|b|k@/etc/$|0", false),
            ("a", "b", "k@/tmp/x", 3, "a|b|k@/tmp/$|E", true),
            ("a", "b", "k@/tmp/x", 0, "a|b|k@/tmp/$|E", false),
            ("a", "b", "k@/tmp/x", 7, "$|b|k@/tmp/$|7", true),
            ("a", "b", "k@/tmp/x", 7, "$|b|k@/tmp/$|8", false),
            ("a", "b", "k@/tmp/x", 7, "a|$|k@/tmp/$|$", true),
            ("a", "b", "k@/tmp/x", 7, "x|b|k@/tmp/$|$", false),
            ("a", "b", "k@/tmp/x", 7, "a|b|missing_at|$", false), // pattern has no '@', expects exact sid
            ("a", "b", "k@/tmp/x", 7, "a|b|k@$|$", true),
            ("a", "b", "k@/tmp/x", 7, "a|b|z@$|$", false),
        ];

        for (aid, eid, sid, ret, pat, exp) in cases {
            let ar = mk_ar(aid, eid, sid, ret);
            assert_eq!(ar.match_eid(pat), exp, "case failed: ar=({aid},{eid},{sid},{ret}) pat={pat}");
        }
    }
}
