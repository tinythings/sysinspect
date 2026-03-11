use crate::{MeNotifyContract, MeNotifyEntrypoint, MeNotifyError};
use mlua::Lua;

#[test]
fn accepts_tick_only_contract() {
    let lua = Lua::new();
    let module = lua
        .load(
            r#"
return {
  tick = function(_ctx) end
}
"#,
        )
        .eval()
        .expect("module should evaluate");

    assert_eq!(
        MeNotifyContract::new(&module, "demo")
            .expect("contract should validate")
            .entrypoint(),
        MeNotifyEntrypoint::Tick
    );
}

#[test]
fn accepts_loop_only_contract() {
    let lua = Lua::new();
    let module = lua
        .load(
            r#"
return {
  loop = function(_ctx) end
}
"#,
        )
        .eval()
        .expect("module should evaluate");

    assert_eq!(
        MeNotifyContract::new(&module, "demo")
            .expect("contract should validate")
            .entrypoint(),
        MeNotifyEntrypoint::Loop
    );
}

#[test]
fn rejects_missing_entrypoint() {
    let lua = Lua::new();
    let module = lua.load("return {}").eval().expect("module should evaluate");
    assert!(matches!(
        MeNotifyContract::new(&module, "demo").expect_err("contract should fail"),
        MeNotifyError::MissingEntrypoint(module) if module == "demo"
    ));
}

#[test]
fn rejects_both_entrypoints() {
    let lua = Lua::new();
    let module = lua
        .load(
            r#"
return {
  tick = function(_ctx) end,
  loop = function(_ctx) end
}
"#,
        )
        .eval()
        .expect("module should evaluate");

    assert!(matches!(
        MeNotifyContract::new(&module, "demo").expect_err("contract should fail"),
        MeNotifyError::AmbiguousEntrypoint(module) if module == "demo"
    ));
}
