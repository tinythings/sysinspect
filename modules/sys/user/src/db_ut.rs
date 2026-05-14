#[cfg(test)]
mod tests {
    use crate::db::*;

    #[test]
    fn parse_valid_passwd_line() {
        let e = parse_passwd("nginx:x:101:101:nginx user:/var/empty:/sbin/nologin").unwrap();
        assert_eq!(e.name, "nginx");
        assert_eq!(e.uid, 101);
        assert_eq!(e.gid, 101);
        assert_eq!(e.gecos, "nginx user");
        assert_eq!(e.home, "/var/empty");
        assert_eq!(e.shell, "/sbin/nologin");
    }

    #[test]
    fn parse_empty_line_is_none() {
        assert!(parse_passwd("").is_none());
    }

    #[test]
    fn format_passwd_roundtrips() {
        let e = PasswdEntry { name: "test".into(), uid: 500, gid: 500, gecos: "".into(), home: "/home/test".into(), shell: "/bin/sh".into() };
        assert_eq!(format_passwd(&e), "test:x:500:500::/home/test:/bin/sh");
    }

    #[test]
    fn parse_valid_group_line() {
        let g = parse_group("wheel:x:10:root,bo").unwrap();
        assert_eq!(g.name, "wheel");
        assert_eq!(g.gid, 10);
        assert_eq!(g.members, vec!["root", "bo"]);
    }

    #[test]
    fn parse_group_no_members() {
        let g = parse_group("nogroup:x:65534:").unwrap();
        assert!(g.members.is_empty());
    }

    #[test]
    fn find_free_uid_skips_used() {
        let lines = vec!["root:x:0:0:root:/root:/bin/bash".to_string(), "daemon:x:1:1:daemon:/usr/sbin:/sbin/nologin".to_string()];
        assert_eq!(find_free_uid(&lines, 0), 2);
    }

    #[test]
    fn find_free_gid_skips_used() {
        let lines = vec!["root:x:0:".to_string(), "staff:x:50:".to_string()];
        assert_eq!(find_free_gid(&lines, 10), 10);
    }
}
