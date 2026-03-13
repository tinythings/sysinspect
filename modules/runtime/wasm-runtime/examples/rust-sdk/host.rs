use serde_json::Value;

#[link(wasm_import_module = "api")]
unsafe extern "C" {
    #[link_name = "header_get"]
    fn sysinspect_header_get(req_ptr: u32, req_len: u32, out_ptr: u32, out_cap: u32) -> i32;
    #[link_name = "header_has"]
    fn sysinspect_header_has(req_ptr: u32, req_len: u32) -> i32;
}

fn read_json<F>(call: F) -> Option<Value>
where
    F: FnOnce(*mut u8, usize) -> i32,
{
    let mut out = vec![0u8; 64 * 1024];
    let n = call(out.as_mut_ptr(), out.len());
    if n < 0 {
        return None;
    }

    serde_json::from_slice::<Value>(&out[..n as usize]).ok()
}

fn escape_pointer_segment(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

fn get(pointer: &str) -> Option<Value> {
    read_json(|out_ptr, out_cap| unsafe {
        sysinspect_header_get(pointer.as_ptr() as u32, pointer.len() as u32, out_ptr as u32, out_cap as u32)
    })
}

fn has_pointer(pointer: &str) -> bool {
    unsafe { sysinspect_header_has(pointer.as_ptr() as u32, pointer.len() as u32) != 0 }
}

/// Return whether a host trait exists.
pub fn has(name: &str) -> bool {
    has_pointer(&format!("/host/traits/{}", escape_pointer_segment(name)))
}

/// Return a host trait value from `host.traits`.
pub fn trait_value(name: &str) -> Option<Value> {
    get(&format!("/host/traits/{}", escape_pointer_segment(name)))
}

/// Return the full `host.paths` object.
pub fn paths() -> Option<Value> {
    get("/host/paths")
}

/// Return a single path value from `host.paths`.
pub fn path_value(name: &str) -> Option<Value> {
    get(&format!("/host/paths/{}", escape_pointer_segment(name)))
}
