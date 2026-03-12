use crate::helpers::getenv;

#[test]
fn getenv_parses_shell_style_pairs() {
    let env = getenv(r#"VAR_ONE="value" VAR_TWO=value VAR_THREE="spaces are supported""#);
    assert_eq!(env["VAR_ONE"], "value");
    assert_eq!(env["VAR_TWO"], "value");
    assert_eq!(env["VAR_THREE"], "spaces are supported");
}
