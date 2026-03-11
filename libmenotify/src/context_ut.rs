use crate::MeNotifyContext;
use mlua::Lua;
use serde_yaml::Value as YamlValue;
use std::time::Duration;

#[test]
fn context_to_lua_keeps_passive_fields() {
    let ctx = MeNotifyContext::new(
        "demo",
        "menotify.demo",
        "demo",
        &["alpha".to_string(), "beta".to_string()],
        &serde_yaml::from_str::<YamlValue>(
            r#"
name: demo
enabled: true
list:
  - one
  - 2
"#,
        )
        .expect("yaml args should parse"),
        Some(Duration::from_secs(5)),
    );

    let lua = Lua::new();
    let tbl = ctx.to_lua(&lua).expect("context should convert to lua");

    assert_eq!(tbl.get::<String>("id").expect("id should exist"), "demo");
    assert_eq!(tbl.get::<String>("listener").expect("listener should exist"), "menotify.demo");
    assert_eq!(tbl.get::<String>("module").expect("module should exist"), "demo");
    assert_eq!(tbl.get::<mlua::Table>("opts").expect("opts should exist").len().expect("len should work"), 2);
    assert_eq!(tbl.get::<mlua::Table>("args").expect("args should exist").get::<String>("name").expect("name should exist"), "demo");
    assert!(tbl.get::<mlua::Table>("args").expect("args should exist").get::<bool>("enabled").expect("enabled should exist"));
    assert_eq!(tbl.get::<f64>("interval").expect("interval should exist"), 5.0);
}
