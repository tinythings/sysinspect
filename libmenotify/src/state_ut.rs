use crate::{MeNotifyContext, MeNotifyState};
use mlua::{Lua, LuaSerdeExt, Value as LuaValue};
use serde_json::json;

#[test]
fn state_store_roundtrips_values() {
    let state = MeNotifyState::new();
    assert!(!state.has("a"));
    state.set("a", json!({"x": 1}));
    assert!(state.has("a"));
    assert_eq!(state.get("a").expect("value should exist"), json!({"x": 1}));
    assert!(state.del("a"));
    assert!(!state.has("a"));
}

#[test]
fn context_state_survives_across_tables() {
    let state = MeNotifyState::new();
    let ctx = MeNotifyContext::with_state("demo", "menotify.demo", "demo", &[], &serde_yaml::Value::Null, None, state.clone());
    let lua = Lua::new();

    let tbl = ctx.to_lua(&lua).expect("context should convert");
    let st = tbl.get::<mlua::Table>("state").expect("state table should exist");
    st.get::<mlua::Function>("set")
        .expect("set function should exist")
        .call::<()>(("last", lua.to_value(&json!("OPS-1")).expect("json should convert")))
        .expect("set should succeed");

    let tbl2 = ctx.to_lua(&lua).expect("second context should convert");
    let st2 = tbl2.get::<mlua::Table>("state").expect("state table should exist");
    let val = st2
        .get::<mlua::Function>("get")
        .expect("get function should exist")
        .call::<LuaValue>("last")
        .and_then(|v| lua.from_value::<serde_json::Value>(v))
        .expect("get should succeed");

    assert_eq!(val, json!("OPS-1"));
}
