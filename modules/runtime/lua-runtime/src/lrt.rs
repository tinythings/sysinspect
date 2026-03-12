use libmodcore::{helpers::RuntimePackageKit, rtdocschema::validate_module_doc, rtspec::RuntimeSpec};
use mlua::{Function, Lua, LuaSerdeExt, Table, Value as LuaValue, Variadic};
use serde_json::Value as JsonValue;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(thiserror::Error, Debug)]
pub enum LuaRuntimeError {
    #[error("lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("sysinspect error: {0}")]
    Sysinspect(#[from] libcommon::SysinspectError),
}

pub type Result<T> = std::result::Result<T, LuaRuntimeError>;

pub struct LuaRuntime {
    lua: Lua,
    scripts_dir: PathBuf,
    logs: Arc<Mutex<Vec<String>>>,
    modulename: Arc<Mutex<String>>,
}

impl LuaRuntime {
    /// Create a new LuaRuntime instance
    /// # Arguments
    /// * `scripts_dir` - Path to the directory containing Lua scripts
    /// # Returns
    /// * `Result<Self>` - Result containing the LuaRuntime instance or an error
    /// # Example
    /// ```no_run
    /// let rt = LuaRuntime::new(PathBuf::from("scripts"))?;
    /// ```
    pub fn new(sharelib_root: PathBuf, enable_native: bool) -> Result<Self> {
        let lua = Lua::new();

        // Runtime configuration
        let lib_dir = sharelib_root.join("lib/runtime/lua/site-lua");
        let globals = lua.globals();
        let package: mlua::Table = globals.get("package")?;

        // Configure native library loader
        if !enable_native {
            package.set("cpath", "")?;
        }

        let mut path = String::new();
        path.push_str(&LuaRuntime::path_fragment(&sharelib_root.join("lib/runtime/lua")));
        path.push(';');
        path.push_str(&LuaRuntime::path_fragment(&lib_dir));

        package.set("path", path)?;

        let rt = Self {
            lua,
            scripts_dir: sharelib_root.join("lib/runtime/lua"),
            logs: Arc::new(Mutex::new(Vec::new())),
            modulename: Arc::new(Mutex::new("Lua module".into())),
        };
        rt.set_logger()?;
        rt.set_packagekit()?;

        Ok(rt)
    }

    // Get scripts path fragment for Lua package.path
    pub fn get_scripts_dir(&self) -> &Path {
        &self.scripts_dir
    }

    fn join_vals(vals: Variadic<LuaValue>) -> String {
        vals.into_iter().map(Self::v2s).collect::<Vec<_>>().join(" ")
    }

    fn v2s(v: LuaValue) -> String {
        match v {
            LuaValue::Nil => "nil".to_string(),
            LuaValue::Boolean(b) => b.to_string(),
            LuaValue::Integer(i) => i.to_string(),
            LuaValue::Number(n) => n.to_string(),
            LuaValue::String(s) => s.to_string_lossy().to_string(),
            // Keep it simple: don't try to serialize tables here.
            other => format!("<lua:{:?}>", other.type_name()),
        }
    }

    /// Set up logging functions in Lua environment
    /// # Returns
    /// * `mlua::Result<()>` - Result of the operation
    fn set_logger(&self) -> mlua::Result<()> {
        fn push_line(logs: &Arc<Mutex<Vec<String>>>, current: &Arc<Mutex<String>>, level: &'static str, msg: String) {
            let module = current.lock().map(|s| s.clone()).unwrap_or_else(|_| "Lua".into());
            let ts = chrono::Local::now().format("%d/%m/%Y %H:%M:%S");
            let line = format!("[{ts}] - {level}: [{module}] {msg}");
            if let Ok(mut g) = logs.lock() {
                g.push(line);
            }
        }

        let globals = self.lua.globals();
        let logtbl: Table = self.lua.create_table()?;

        // error(...)
        {
            let logs = self.logs.clone();
            let m = self.modulename.clone();
            logtbl.set(
                "error",
                self.lua.create_function(move |_, vals: Variadic<LuaValue>| {
                    push_line(&logs, &m, "ERROR", LuaRuntime::join_vals(vals));
                    Ok(())
                })?,
            )?;
        }

        // warn(...)
        {
            let logs = self.logs.clone();
            let m = self.modulename.clone();
            logtbl.set(
                "warn",
                self.lua.create_function(move |_, vals: Variadic<LuaValue>| {
                    push_line(&logs, &m, "WARN", LuaRuntime::join_vals(vals));
                    Ok(())
                })?,
            )?;
        }

        // info(...)
        {
            let logs = self.logs.clone();
            let m = self.modulename.clone();
            logtbl.set(
                "info",
                self.lua.create_function(move |_, vals: Variadic<LuaValue>| {
                    push_line(&logs, &m, "INFO", LuaRuntime::join_vals(vals));
                    Ok(())
                })?,
            )?;
        }

        // debug(...)
        {
            let logs = self.logs.clone();
            let m = self.modulename.clone();
            logtbl.set(
                "debug",
                self.lua.create_function(move |_, vals: Variadic<LuaValue>| {
                    push_line(&logs, &m, "DEBUG", LuaRuntime::join_vals(vals));
                    Ok(())
                })?,
            )?;
        }

        globals.set("log", logtbl)?;
        Ok(())
    }

    /// Set up PackageKit helper functions in Lua environment.
    /// # Returns
    /// * `mlua::Result<()>` - Result of the operation
    fn set_packagekit(&self) -> mlua::Result<()> {
        let globals = self.lua.globals();
        let pktbl: Table = self.lua.create_table()?;

        pktbl.set("available", self.lua.create_function(move |_, ()| Ok(RuntimePackageKit::available()))?)?;

        pktbl.set(
            "status",
            self.lua.create_function(move |lua, ()| {
                RuntimePackageKit::status().map_err(|err| mlua::Error::runtime(err.to_string())).and_then(|status| lua.to_value(&status))
            })?,
        )?;

        pktbl.set(
            "history",
            self.lua.create_function(move |lua, (names, count): (Vec<String>, Option<u32>)| {
                RuntimePackageKit::history(names, count.unwrap_or(10))
                    .map_err(|err| mlua::Error::runtime(err.to_string()))
                    .and_then(|history| lua.to_value(&history))
            })?,
        )?;

        pktbl.set(
            "packages",
            self.lua.create_function(move |lua, ()| {
                RuntimePackageKit::packages().map_err(|err| mlua::Error::runtime(err.to_string())).and_then(|packages| lua.to_value(&packages))
            })?,
        )?;

        pktbl.set(
            "install",
            self.lua.create_function(move |lua, names: Vec<String>| {
                RuntimePackageKit::install(names).map_err(|err| mlua::Error::runtime(err.to_string())).and_then(|result| lua.to_value(&result))
            })?,
        )?;

        globals.set("packagekit", pktbl)?;
        Ok(())
    }

    // Lua package.path uses ; separated patterns with ?
    // Typical: /path/?.lua;/path/?/init.lua
    fn path_fragment(dir: &Path) -> String {
        let d = dir.to_string_lossy();
        format!("{d}/?.lua;{d}/?/init.lua")
    }

    /// Call Lua module's run(req) function
    /// # Arguments
    /// * `code` - Lua module code string
    /// * `req` - JSON request value
    /// # Returns
    /// * `JsonValue` - JSON response value
    /// # Errors
    /// * `LuaRuntimeError` - If the module call fails
    /// # Example
    /// ```no_run
    /// let rt = LuaRuntime::new(PathBuf::from("scripts"))?;
    /// let resp = rt.call_module(r#"return { run = function(req) return { message = "Hello, " .. req.name } end }"#, &serde_json::json!({ "name": "World" }))?;
    /// println!("{}", serde_json::to_string_pretty(&resp).unwrap());
    /// ```
    pub fn call_module(&self, modname: &str, code: &str, req: &JsonValue, with_logs: bool) -> Result<JsonValue> {
        // clear per-call log buffer, in case lrt is called multiple times
        if let Ok(mut g) = self.logs.lock() {
            g.clear();
        }

        // Set module name for logging
        if let Ok(mut m) = self.modulename.lock() {
            *m = modname.to_string();
        }

        // Tell Lua module its name
        self.lua.globals().set("__module_name", modname)?;

        let module: Table = self.lua.load(code).eval()?;
        let run: Function =
            module.get(RuntimeSpec::MainEntryFunction.to_string()).map_err(|_| mlua::Error::runtime("Lua module must export run(req) function!"))?;

        let lua_req = self.lua.to_value(req)?;
        let result: LuaValue = run.call(lua_req)?;

        // Parse module return into JSON "data"
        let data: JsonValue = match result {
            LuaValue::Table(t) => self.lua.from_value(LuaValue::Table(t))?,
            LuaValue::String(s) => serde_json::from_str(&s.to_str()?).map_err(|e| mlua::Error::runtime(e.to_string()))?,
            _ => return Err(mlua::Error::runtime("Lua run() must return table or JSON string").into()),
        };

        // Grab buffered logs
        let logs = if with_logs { if let Ok(g) = self.logs.lock() { g.clone() } else { Vec::new() } } else { Vec::new() };

        // Return { data, logs }
        Ok(serde_json::json!({
            RuntimeSpec::DataSectionField.to_string(): data,
            RuntimeSpec::LogsSectionField.to_string(): logs
        }))
    }

    /// Get module documentation from Lua code
    /// # Arguments
    /// * `code` - Lua module code string
    /// # Returns
    /// * `JsonValue` - Module documentation as JSON value
    /// # Errors
    /// * `LuaRuntimeError` - If the documentation retrieval or validation fails
    /// # Example
    /// ```no_run
    /// let rt = LuaRuntime::new(PathBuf::from("scripts"))?;
    /// let doc = rt.module_doc(r#"return { documentation = { name = "My Module", description = "This is a test module." } }"#)?;
    /// println!("{}", serde_json::to_string_pretty(&doc).unwrap());
    /// ```
    pub fn module_doc(&self, code: &str) -> Result<JsonValue> {
        let module: Table = self.lua.load(code).eval()?;
        let doc: LuaValue = module.get(RuntimeSpec::DocumentationFunction.to_string())?;

        let json = match doc {
            LuaValue::Table(t) => self.lua.from_value(LuaValue::Table(t))?,
            LuaValue::Nil => serde_json::json!({}),
            _ => return Err(mlua::Error::runtime("module doc must be a table").into()),
        };

        validate_module_doc(&json)?;

        Ok(json)
    }
}
