use libmodcore::{rtdocschema::validate_module_doc, rtspec::RuntimeSpec};
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use serde_json::Value as JsonValue;
use std::path::{Path, PathBuf};

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
    pub fn new(sharelib_root: PathBuf) -> Result<Self> {
        let lua = Lua::new();

        // Runtime configuration
        let lib_dir = sharelib_root.join("lib/runtime/site-lua");
        let globals = lua.globals();
        let package: mlua::Table = globals.get("package")?;
        package.set("cpath", "")?; // disable native module loading

        let mut path = String::new();
        path.push_str(&LuaRuntime::path_fragment(&sharelib_root));
        path.push(';');
        path.push_str(&LuaRuntime::path_fragment(&lib_dir));

        package.set("path", path)?;

        // Optional: disable native module loading unless you want it
        // package.set("cpath", "")?;

        // ----- sys.echo to stderr -----
        let sys: mlua::Table = lua.create_table()?;
        sys.set(
            "echo",
            lua.create_function(|_, msg: String| {
                eprintln!("[lua] {}", msg);
                Ok(())
            })?,
        )?;
        globals.set("sys", sys)?;

        Ok(Self { lua, scripts_dir: sharelib_root })
    }

    // Get scripts path fragment for Lua package.path
    pub fn get_scripts_dir(&self) -> &Path {
        &self.scripts_dir
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
    pub fn call_module(&self, code: &str, req: &JsonValue) -> Result<JsonValue> {
        let module: Table = self.lua.load(code).eval()?;
        let run: Function =
            module.get(RuntimeSpec::MainEntryFunction.to_string()).map_err(|_| mlua::Error::runtime("Lua module must export run(req) function!"))?;

        let lua_req = self.lua.to_value(req)?;
        let result: Value = run.call(lua_req)?;

        // Accept table OR string
        match result {
            Value::Table(t) => {
                let json: JsonValue = self.lua.from_value(Value::Table(t))?;
                Ok(json)
            }
            Value::String(s) => {
                let parsed: JsonValue = serde_json::from_str(&s.to_str()?).map_err(|e| mlua::Error::runtime(e.to_string()))?;
                Ok(parsed)
            }
            _ => Err(mlua::Error::runtime("Lua run() must return table or JSON string").into()),
        }
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
        let doc: Value = module.get(RuntimeSpec::DocumentationFunction.to_string())?;

        let json = match doc {
            Value::Table(t) => self.lua.from_value(Value::Table(t))?,
            Value::Nil => serde_json::json!({}),
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
