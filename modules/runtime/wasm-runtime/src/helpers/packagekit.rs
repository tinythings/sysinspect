use libmodcore::helpers::RuntimePackageKit;
use serde::Deserialize;
use wasmruntime::{API_NAMESPACE, HostState, output_region, request_bytes, write_error, write_json};
use wasmtime::{Caller, Extern, Linker, Memory};

#[derive(Debug, Deserialize)]
struct PackageKitNamesReq {
    #[serde(default)]
    names: Vec<String>,
    #[serde(default)]
    count: Option<u32>,
}

/// Register PackageKit helper imports into the Wasm runtime linker.
///
/// Arguments:
/// * `linker` - Wasm runtime linker instance to extend.
///
/// Returns:
/// * `anyhow::Result<()>` - Ok when helper imports were added.
pub fn register(linker: &mut Linker<HostState>) -> anyhow::Result<()> {
    linker
        .func_wrap(API_NAMESPACE, "packagekit_available", || -> i32 {
            if RuntimePackageKit::available() { 1 } else { 0 }
        })
        .map_err(|err| anyhow::anyhow!("Failed to register Wasm PackageKit availability helper: {err}"))?;

    linker
        .func_wrap(API_NAMESPACE, "packagekit_status", |mut caller: Caller<'_, HostState>, out_ptr: i32, out_cap: i32| -> i32 {
            let mem: Memory = match caller.get_export("memory") {
                Some(Extern::Memory(m)) => m,
                _ => return -2,
            };
            let Some((out_ptr, out_cap)) = output_region(&caller, &mem, out_ptr, out_cap) else {
                return -2;
            };

            match RuntimePackageKit::status() {
                Ok(status) => {
                    write_json(&mem, &mut caller, out_ptr, out_cap, &serde_json::to_value(status).unwrap_or_else(|_| serde_json::json!({})))
                }
                Err(err) => write_error(&mem, &mut caller, out_ptr, out_cap, &err.to_string()),
            }
        })
        .map_err(|err| anyhow::anyhow!("Failed to register Wasm PackageKit status helper: {err}"))?;

    linker
        .func_wrap(API_NAMESPACE, "packagekit_packages", |mut caller: Caller<'_, HostState>, out_ptr: i32, out_cap: i32| -> i32 {
            let mem: Memory = match caller.get_export("memory") {
                Some(Extern::Memory(m)) => m,
                _ => return -2,
            };
            let Some((out_ptr, out_cap)) = output_region(&caller, &mem, out_ptr, out_cap) else {
                return -2;
            };

            match RuntimePackageKit::packages() {
                Ok(packages) => write_json(&mem, &mut caller, out_ptr, out_cap, &packages),
                Err(err) => write_error(&mem, &mut caller, out_ptr, out_cap, &err.to_string()),
            }
        })
        .map_err(|err| anyhow::anyhow!("Failed to register Wasm PackageKit packages helper: {err}"))?;

    linker
        .func_wrap(
            API_NAMESPACE,
            "packagekit_history",
            |mut caller: Caller<'_, HostState>, req_ptr: i32, req_len: i32, out_ptr: i32, out_cap: i32| -> i32 {
                let mem: Memory = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return -2,
                };
                let Some(req_bytes) = request_bytes(&caller, &mem, req_ptr, req_len) else {
                    return -2;
                };
                let Some((out_ptr, out_cap)) = output_region(&caller, &mem, out_ptr, out_cap) else {
                    return -2;
                };

                let req: PackageKitNamesReq = match serde_json::from_slice(req_bytes) {
                    Ok(req) => req,
                    Err(err) => return write_error(&mem, &mut caller, out_ptr, out_cap, &format!("invalid PackageKit history request: {err}")),
                };

                match RuntimePackageKit::history(req.names, req.count.unwrap_or(10)) {
                    Ok(history) => write_json(&mem, &mut caller, out_ptr, out_cap, &history),
                    Err(err) => write_error(&mem, &mut caller, out_ptr, out_cap, &err.to_string()),
                }
            },
        )
        .map_err(|err| anyhow::anyhow!("Failed to register Wasm PackageKit history helper: {err}"))?;

    linker
        .func_wrap(
            API_NAMESPACE,
            "packagekit_install",
            |mut caller: Caller<'_, HostState>, req_ptr: i32, req_len: i32, out_ptr: i32, out_cap: i32| -> i32 {
                let mem: Memory = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return -2,
                };
                let Some(req_bytes) = request_bytes(&caller, &mem, req_ptr, req_len) else {
                    return -2;
                };
                let Some((out_ptr, out_cap)) = output_region(&caller, &mem, out_ptr, out_cap) else {
                    return -2;
                };

                let req: PackageKitNamesReq = match serde_json::from_slice(req_bytes) {
                    Ok(req) => req,
                    Err(err) => return write_error(&mem, &mut caller, out_ptr, out_cap, &format!("invalid PackageKit install request: {err}")),
                };

                match RuntimePackageKit::install(req.names) {
                    Ok(result) => write_json(&mem, &mut caller, out_ptr, out_cap, &result),
                    Err(err) => write_error(&mem, &mut caller, out_ptr, out_cap, &err.to_string()),
                }
            },
        )
        .map_err(|err| anyhow::anyhow!("Failed to register Wasm PackageKit install helper: {err}"))?;

    linker
        .func_wrap(
            API_NAMESPACE,
            "packagekit_remove",
            |mut caller: Caller<'_, HostState>, req_ptr: i32, req_len: i32, out_ptr: i32, out_cap: i32| -> i32 {
                let mem: Memory = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return -2,
                };
                let Some(req_bytes) = request_bytes(&caller, &mem, req_ptr, req_len) else {
                    return -2;
                };
                let Some((out_ptr, out_cap)) = output_region(&caller, &mem, out_ptr, out_cap) else {
                    return -2;
                };

                let req: PackageKitNamesReq = match serde_json::from_slice(req_bytes) {
                    Ok(req) => req,
                    Err(err) => return write_error(&mem, &mut caller, out_ptr, out_cap, &format!("invalid PackageKit remove request: {err}")),
                };

                match RuntimePackageKit::remove(req.names) {
                    Ok(result) => write_json(&mem, &mut caller, out_ptr, out_cap, &result),
                    Err(err) => write_error(&mem, &mut caller, out_ptr, out_cap, &err.to_string()),
                }
            },
        )
        .map_err(|err| anyhow::anyhow!("Failed to register Wasm PackageKit remove helper: {err}"))?;

    linker
        .func_wrap(
            API_NAMESPACE,
            "packagekit_upgrade",
            |mut caller: Caller<'_, HostState>, req_ptr: i32, req_len: i32, out_ptr: i32, out_cap: i32| -> i32 {
                let mem: Memory = match caller.get_export("memory") {
                    Some(Extern::Memory(m)) => m,
                    _ => return -2,
                };
                let Some(req_bytes) = request_bytes(&caller, &mem, req_ptr, req_len) else {
                    return -2;
                };
                let Some((out_ptr, out_cap)) = output_region(&caller, &mem, out_ptr, out_cap) else {
                    return -2;
                };

                let req: PackageKitNamesReq = match serde_json::from_slice(req_bytes) {
                    Ok(req) => req,
                    Err(err) => return write_error(&mem, &mut caller, out_ptr, out_cap, &format!("invalid PackageKit upgrade request: {err}")),
                };

                match RuntimePackageKit::upgrade(req.names) {
                    Ok(result) => write_json(&mem, &mut caller, out_ptr, out_cap, &result),
                    Err(err) => write_error(&mem, &mut caller, out_ptr, out_cap, &err.to_string()),
                }
            },
        )
        .map_err(|err| anyhow::anyhow!("Failed to register Wasm PackageKit upgrade helper: {err}"))?;

    Ok(())
}
