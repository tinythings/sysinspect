use futures::executor;
use libcommon::SysinspectError;
use libmodcore::{response::ModResponse, rtspec::RuntimeSpec, runtime::ModRequest};
use serde_json::Value;
use wasmruntime::cfg::WasmConfig;

/// Config entry for path to shared library
const PATH_SHARELIB: &str = "path.sharelib"; // Root
const PATH_USERLETS: &str = "lib/runtime/wasm";

pub struct WasmRuntime {
    rq: ModRequest,
    rt: wasmruntime::WasmRuntime,
}

impl WasmRuntime {
    pub fn new(rq: &ModRequest) -> Result<Self, SysinspectError> {
        let wcfg = Self::get_wcfg(rq)?;
        let rt = match wasmruntime::WasmRuntime::new(wcfg.clone()) {
            Err(err) => {
                return Err(SysinspectError::ConfigError(format!("Failed to initialize Wasm runtime: {err}")));
            }
            Ok(rt) => rt,
        };

        Ok(WasmRuntime { rq: rq.clone(), rt })
    }

    /// Get sharelib path
    fn get_sharelib(rq: &ModRequest) -> String {
        let sharelib = rq.config().get(PATH_SHARELIB).and_then(|v| v.as_string()).unwrap_or_default().trim_end_matches('/').to_string();
        if !sharelib.is_empty() { format!("{sharelib}/{PATH_USERLETS}") } else { String::new() }
    }

    /// Get Wasm config
    fn get_wcfg(rq: &ModRequest) -> Result<WasmConfig, SysinspectError> {
        let mut wcfg = WasmConfig::default();

        // Go modules require guest path to be explicitly "/"
        // TinyGo is permissive and "." is enough, but native Go is not.
        wcfg.set_guest_path("/");
        if let Err(err) = wcfg.set_host_path("/") {
            return Err(SysinspectError::ConfigError(format!("Failed to set default host path: {err}")));
        }

        let sharelib = Self::get_sharelib(rq);
        if sharelib.is_empty() {
            return Err(SysinspectError::ConfigError(format!("Config entry \"{PATH_SHARELIB}\" is missing or empty")));
        }
        wcfg.set_rootdir(sharelib);

        rq.args().get("write-dir").and_then(|v| v.as_string()).map(|p| p.trim_end_matches('/').to_string()).filter(|p| !p.is_empty()).map_or(
            Ok(()),
            |p| {
                if !p.starts_with('/') {
                    return Err(SysinspectError::ConfigError("Argument \"write-dir\" must be an absolute path".to_string()));
                }

                if let Err(err) = wcfg.set_host_path(p) {
                    return Err(SysinspectError::ConfigError(format!("Failed to set host path: {err}")));
                }

                Ok(())
            },
        )?;

        wcfg.set_allow_write(true);
        Ok(wcfg)
    }

    pub fn get_wasm_modules(&self) -> Result<Vec<String>, SysinspectError> {
        let mods: Vec<String> = match self.rt.objects() {
            Err(err) => {
                return Err(SysinspectError::ConfigError(format!("Failed to list userlets: {err}")));
            }
            Ok(uls) => uls,
        };
        Ok(mods)
    }

    pub fn run(&self) -> ModResponse {
        let mut r = ModResponse::new_cm();
        // Get Wasm modules
        let wmod = match self.get_wasm_modules() {
            Err(err) => {
                r.set_message(&format!("Failed to get WASM modules: {err}"));
                r.set_retcode(3);
                return r;
            }
            Ok(uls) => uls,
        };

        // Calling
        let mod_id = self.rq.args_all().get("rt.mod").and_then(|v| v.as_string()).unwrap_or_default();
        if !wmod.contains(&mod_id.to_string()) {
            r.set_message(&format!("Module \"{mod_id}\" was not found"));
            r.set_retcode(4);
            return r;
        }

        // `run` is async and returns a Future; we must drive it to completion.
        let mut out = match executor::block_on(self.rt.run(
            &mod_id,
            self.rq.options().iter().map(|v| v.as_string().unwrap_or_default()).filter(|s| !s.is_empty()).collect(),
            self.rq.args().into_iter().map(|(k, v)| (k, v.into())).collect(),
            vec![], // This is incoming NDJSON (usually for databases). Unused in this scenario.
        )) {
            Err(err) => {
                r.set_message(&format!("Failed to run module \"{mod_id}\": {err}"));
                r.set_retcode(5);
                return r;
            }
            Ok(val) => val,
        };

        if let Value::Object(ref mut map) = out
            && let Some(v) = map.remove("__module-logs")
        {
            map.insert(RuntimeSpec::LogsSectionField.to_string(), v);
        }

        r.set_message("Wasm runtime executed successfully");
        r.set_retcode(0);
        if let Err(err) = r.set_data(out) {
            r.set_message(&format!("Failed to set response data: {err}"));
            r.set_retcode(6);
            return r;
        }

        r
    }
}
