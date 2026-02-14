mod profile_match_test {
    use libsensors::sspec::SensorSpec;
    use std::str::FromStr;

    #[test]
    fn test_matches_profile_default_when_missing() {
        let y = r#"
sensors:
  a:
    listener: file
"#;

        let mut spec = SensorSpec::from_str(y).unwrap();
        let items = spec.items();
        let c = items.get("a").unwrap();

        let profiles = vec!["default".to_string()];
        assert!(c.matches_profile(&profiles));

        let profiles2 = vec!["banana".to_string()];
        assert!(!c.matches_profile(&profiles2));
    }

    #[test]
    fn test_matches_profile_case_insensitive() {
        let y = r#"
sensors:
  a:
    listener: file
    profile: [BrownStinkyBanana]
"#;

        let mut spec = SensorSpec::from_str(y).unwrap();
        let items = spec.items();
        let c = items.get("a").unwrap();

        let profiles = vec!["brownstinkybanana".to_string()];
        assert!(c.matches_profile(&profiles));

        let profiles2 = vec!["BROWNSTINKYBANANA".to_string()];
        assert!(c.matches_profile(&profiles2));

        let profiles3 = vec!["default".to_string()];
        assert!(!c.matches_profile(&profiles3));
    }
}
