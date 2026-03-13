use super::actions::Action;

#[test]
fn runtime_virtual_namespace_is_recognised() {
    assert_eq!(Action::runtime_dispatch("lua.reader"), Some(("runtime.lua", "reader".to_string())));
    assert_eq!(Action::runtime_dispatch("py3.nested.reader"), Some(("runtime.py3", "nested.reader".to_string())));
    assert_eq!(Action::runtime_dispatch("wasm.caller"), Some(("runtime.wasm", "caller".to_string())));
    assert_eq!(Action::runtime_dispatch("sys.run"), None);
    assert_eq!(Action::runtime_dispatch("lua."), None);
}
