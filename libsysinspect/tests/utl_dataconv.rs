#[cfg(test)]
mod dataconv_test {
    use libsysinspect::util::dataconv;
    use serde_yaml::{self, Value};
    use std::collections::HashMap;

    #[test]
    fn test_dataconv_int() {
        let m = serde_yaml::from_str::<HashMap<String, Value>>("foo: 1").unwrap();
        let v = m.get("foo").unwrap();
        let i = dataconv::as_int(Some(v).cloned());
        assert!(i == 1);
    }

    #[test]
    fn test_dataconv_bool() {
        let m = serde_yaml::from_str::<HashMap<String, Value>>("foo: true").unwrap();
        let v = m.get("foo").unwrap();
        let i = dataconv::as_bool(Some(v).cloned());
        assert!(i);
    }

    #[test]
    fn test_dataconv_str() {
        let m = serde_yaml::from_str::<HashMap<String, Value>>("foo: \"Darth Vader\"").unwrap();
        let v = m.get("foo").unwrap();
        let i = dataconv::as_str(Some(v).cloned());
        assert!(i.eq("Darth Vader"));
    }

    #[test]
    fn test_dataconv_str_opt() {
        let m = serde_yaml::from_str::<HashMap<String, Value>>("foo: \"Darth Vader\"").unwrap();
        let v = m.get("foo").unwrap();
        let i = dataconv::as_str_opt(Some(v).cloned());
        assert!(i.is_some(), "Data must contain something");
        assert!(i.unwrap_or_default().eq("Darth Vader"), "Data must be a Luke's father");
    }

    #[test]
    fn test_dataconv_bool_opt() {
        let m = serde_yaml::from_str::<HashMap<String, Value>>("foo: true").unwrap();
        let v = m.get("foo").unwrap();
        let i = dataconv::as_bool_opt(Some(v).cloned());
        assert!(i.is_some(), "Data must contain something");
        assert!(i.unwrap_or(false), "Data must be true");
    }

    #[test]
    fn test_dataconv_int_opt() {
        let m = serde_yaml::from_str::<HashMap<String, Value>>("foo: 1").unwrap();
        let v = m.get("foo").unwrap();
        let i = dataconv::as_int_opt(Some(v).cloned());
        assert!(i.is_some(), "Data must contain something");
        assert!(i.unwrap_or(0) == 1, "Data must be 1");
    }

    #[test]
    fn test_dataconv_str_list() {
        let m = serde_yaml::from_str::<HashMap<String, Value>>("foo: \"bar,spam,baz,toto\"").unwrap();
        let v = m.get("foo").unwrap();
        let l = dataconv::as_str_list(Some(v).cloned());
        assert!(l.len() == 4, "Data length must be 4");
        assert!(
            l == vec!["bar".to_string(), "spam".to_string(), "baz".to_string(), "toto".to_string()],
            "Vector must be the same"
        );
    }

    #[test]
    fn test_dataconv_str_list_opt() {
        let m = serde_yaml::from_str::<HashMap<String, Value>>("foo: \"bar,spam,baz,toto\"").unwrap();
        let v = m.get("foo").unwrap();
        let l = dataconv::as_str_list_opt(Some(v).cloned());
        assert!(l.is_some(), "Data must contain something");
        assert!(l.clone().unwrap().len() == 4, "Data length must be 4");
        assert!(
            l.unwrap() == vec!["bar".to_string(), "spam".to_string(), "baz".to_string(), "toto".to_string()],
            "Vector must be the same"
        );
    }

    #[test]
    fn test_dataconv_to_str() {
        let m = serde_yaml::from_str::<HashMap<String, Value>>("foo: \"bar,spam,baz,toto\"").unwrap();
        let v = m.get("foo").unwrap();
        let s = dataconv::to_string(Some(v).cloned());
        assert!(s.is_some(), "Data must contain something");
        let s = s.unwrap();
        assert!(s.eq("bar,spam,baz,toto"));
    }
}
