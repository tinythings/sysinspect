use libmodcore::{rtdocschema::validate_module_doc, rtspec::RuntimeSpec};
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
    Sysinspect(#[from] libsysinspect::SysinspectError),

    #[error("failed to read lua file '{path}': {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{msg}: {source}")]
    Context {
        msg: String,
        #[source]
        source: Box<LuaRuntimeError>,
    },
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
        let lib_dir = sharelib_root.join("lib/runtime/lua54/site-lua");
        let globals = lua.globals();
        let package: mlua::Table = globals.get("package")?;

        // Configure native library loader
        if !enable_native {
            package.set("cpath", "")?;
        }

        let mut path = String::new();
        path.push_str(&LuaRuntime::path_fragment(&sharelib_root.join("lib/runtime/lua54")));
        path.push(';');
        path.push_str(&LuaRuntime::path_fragment(&lib_dir));

        package.set("path", path)?;

        let rt = Self {
            lua,
            scripts_dir: sharelib_root.join("lib/runtime/lua54"),
            logs: Arc::new(Mutex::new(Vec::new())),
            modulename: Arc::new(Mutex::new("Lua module".into())),
        };
        rt.set_logger()?;

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

    // Lua package.path uses ; separated patterns with ?
    // Typical: /path/?.lua;/path/?/init.lua
    fn path_fragment(dir: &Path) -> String {
        let d = dir.to_string_lossy();
        format!("{d}/?.lua;{d}/?/init.lua")
    }

    /// Execute Lua code string
    /// # Arguments
    /// * `code` - Lua code string
    /// # Returns
    /// * `Result<()>` - Result of the execution
    /// # Errors
    /// * `LuaRuntimeError` - If the execution fails
    /// # Example
    /// ```no_run
    /// let rt = LuaRuntime::new(PathBuf::from("scripts"))?;
    /// rt.exec_str(r#"print("Hello, Lua!")"#)?;
    /// ```
    pub fn exec_str(&self, code: &str) -> Result<()> {
        self.lua.load(code).exec()?;
        Ok(())
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

    pub fn exec_file(&self, path: &str) -> Result<()> {
        let code = std::fs::read_to_string(path).map_err(|e| LuaRuntimeError::ReadFile { path: path.to_string(), source: e })?;

        self.exec_str(&code).map_err(|e| LuaRuntimeError::Context { msg: format!("lua exec_file failed: {path}"), source: Box::new(e) })
    }

    /// Call a Lua function with arguments
    /// # Arguments
    /// * `name` - Function name
    /// * `args` - Function arguments
    /// # Returns
    /// * `R` - Return type of the function
    /// # Errors
    /// * `LuaRuntimeError` - If the function call fails
    /// # Example
    /// ```no_run
    /// let rt = LuaRuntime::new(PathBuf::from("scripts"))?;
    /// rt.exec_str(r#"function add(a, b) return a + b end"#)?;
    /// let result: i64 = rt.call_fn("add", (2, 3))?;
    /// assert_eq!(result, 5);
    /// ```
    pub fn call_fn<R: mlua::FromLua>(&self, name: &str, args: impl mlua::IntoLuaMulti) -> Result<R> {
        let globals = self.lua.globals();
        let f: mlua::Function = globals.get(name)?;
        Ok(f.call(args)?)
    }

    /// Set a global variable in Lua
    /// # Arguments
    /// * `key` - Variable name
    /// * `val` - Variable value
    /// # Returns
    /// * `Result<()>` - Result of the operation
    /// # Errors
    /// * `LuaRuntimeError` - If setting the global variable fails
    /// # Example
    /// ```no_run
    /// let rt = LuaRuntime::new(PathBuf::from("scripts"))?;
    /// rt.set_global("my_var", 42)?;
    /// let value: i64 = rt.get_global("my_var")?;
    /// assert_eq!(value, 42);
    /// ```
    pub fn set_global(&self, key: &str, val: impl mlua::IntoLua) -> Result<()> {
        let v = val.into_lua(&self.lua)?;
        self.lua.globals().set(key, v)?;
        Ok(())
    }

    /// Get a global variable from Lua
    /// # Arguments
    /// * `key` - Variable name
    /// # Returns
    /// * `Result<T>` - Value of the variable
    /// # Errors
    /// * `LuaRuntimeError` - If getting the global variable fails
    /// # Example
    /// ```no_run
    /// let rt = LuaRuntime::new(PathBuf::from("scripts"))?;
    /// rt.set_global("my_var", 42)?;
    /// let value: i64 = rt.get_global("my_var")?;
    /// assert_eq!(value, 42);
    /// ```
    pub fn get_global<T: mlua::FromLua>(&self, key: &str) -> Result<T> {
        Ok(self.lua.globals().get(key)?)
    }
}
