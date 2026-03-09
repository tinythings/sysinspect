use libcommon::SysinspectError;
use libmodcore::{rtdocschema::validate_module_doc, rtspec::RuntimeSpec};
use rustpython::InterpreterBuilderExt;
use rustpython_vm::{
    Interpreter, PyObjectRef, PyResult, Settings, VirtualMachine,
    builtins::PyStr,
    compiler::Mode::Exec,
    function::{FuncArgs, KwArgs},
    pymodule,
};
use serde_json::Value as JsonValue;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex},
};

#[derive(Default)]
struct PyLoggerState {
    logs: Vec<String>,
    modulename: String,
}

static PY_LOG_STATE: LazyLock<Mutex<PyLoggerState>> = LazyLock::new(|| Mutex::new(PyLoggerState::default()));

/// Set current Python module name for forwarded log messages
/// # Arguments
/// * `modname` - Current Python module name
fn set_log_modulename(modname: &str) {
    if let Ok(mut g) = PY_LOG_STATE.lock() {
        g.modulename = modname.to_string();
    }
}

/// Clear buffered Python log lines before a module call
fn clear_log_buffer() {
    if let Ok(mut g) = PY_LOG_STATE.lock() {
        g.logs.clear();
    }
}

/// Get buffered Python log lines
/// # Returns
/// * `Vec<String>` - Collected log lines
fn get_log_buffer() -> Vec<String> {
    if let Ok(g) = PY_LOG_STATE.lock() { g.logs.clone() } else { Vec::new() }
}

/// Push a single formatted log line into the runtime buffer
/// # Arguments
/// * `level` - Log level
/// * `msg` - Log message
fn push_log_line(level: &str, msg: &str) {
    if let Ok(mut g) = PY_LOG_STATE.lock() {
        let ts = chrono::Local::now().format("%d/%m/%Y %H:%M:%S");
        let modulename = if g.modulename.is_empty() { "Python module".to_string() } else { g.modulename.clone() };
        g.logs.push(format!("[{ts}] - {level}: [{modulename}] {msg}"));
    }
}

#[pymodule]
mod rtlog {
    use super::push_log_line;
    use rustpython_vm::{PyResult, VirtualMachine};

    /// Write a log entry from Python code into SysInspect runtime buffer
    /// # Arguments
    /// * `level` - Log level
    /// * `msg` - Log message text
    /// # Returns
    /// * `PyResult<()>` - Result of the operation
    #[pyfunction]
    fn write(level: String, msg: String, _vm: &VirtualMachine) -> PyResult<()> {
        push_log_line(level.trim(), msg.trim());
        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Py3RuntimeError {
    #[error("python runtime error: {0}")]
    Runtime(#[from] SysinspectError),

    #[error("python vm error: {0}")]
    Vm(String),

    #[error("failed to read python file '{path}': {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, Py3RuntimeError>;

pub struct Py3Runtime {
    itp: Interpreter,
    scripts_dir: PathBuf,
    lib_dir: PathBuf,
    logs: Arc<Mutex<Vec<String>>>,
    modulename: Arc<Mutex<String>>,
}

impl Py3Runtime {
    /// Create a new Py3Runtime instance
    /// # Arguments
    /// * `sharelib_root` - Path to SysInspect sharelib root
    /// # Returns
    /// * `Result<Self>` - Configured Python runtime instance
    pub fn new(sharelib_root: PathBuf) -> Result<Self> {
        let scripts_dir = sharelib_root.join("lib/runtime/python3");
        let lib_dir = scripts_dir.join("site-packages");
        let mut cfg = Settings::default();
        cfg.path_list.push(scripts_dir.to_string_lossy().to_string());
        cfg.path_list.push(lib_dir.to_string_lossy().to_string());

        let builder = rustpython::Interpreter::builder(cfg);
        let rtlog_def = rtlog::module_def(&builder.ctx);
        let itp = builder.init_stdlib().add_native_module(rtlog_def).build();

        Ok(Self { itp, scripts_dir, lib_dir, logs: Arc::new(Mutex::new(Vec::new())), modulename: Arc::new(Mutex::new("Python module".to_string())) })
    }

    /// Get Python runtime scripts directory
    /// # Returns
    /// * `&Path` - Path to Python runtime modules
    pub fn get_scripts_dir(&self) -> &Path {
        &self.scripts_dir
    }

    /// Append runtime-specific Python paths to `sys.path`
    /// # Arguments
    /// * `vm` - RustPython virtual machine
    /// # Returns
    /// * `Result<()>` - Result of the operation
    fn load_pylib(&self, vm: &VirtualMachine) -> Result<()> {
        for path in [&self.scripts_dir, &self.lib_dir] {
            if let Ok(sysmod) = vm.import("sys", 0) {
                if let Ok(syspath) = sysmod.get_attr("path", vm) {
                    if let Err(err) = vm.call_method(&syspath, "append", (path.to_string_lossy().to_string(),)) {
                        return Err(Py3RuntimeError::Vm(format!("Failed to append Python path {}: {err:?}", path.display())));
                    }
                } else {
                    return Err(Py3RuntimeError::Vm("Unable to access sys.path".to_string()));
                }
            } else {
                return Err(Py3RuntimeError::Vm("Unable to import sys module".to_string()));
            }
        }

        Ok(())
    }

    /// Convert JSON value into a Python object
    /// # Arguments
    /// * `vm` - RustPython virtual machine
    /// * `value` - JSON value to convert
    /// # Returns
    /// * `PyResult<PyObjectRef>` - Converted Python object
    fn from_json(vm: &VirtualMachine, value: JsonValue) -> PyResult<PyObjectRef> {
        Ok(match value {
            JsonValue::Null => vm.ctx.none(),
            JsonValue::Bool(b) => vm.ctx.new_bool(b).into(),
            JsonValue::Number(num) => {
                if let Some(i) = num.as_i64() {
                    vm.ctx.new_int(i).into()
                } else if let Some(f) = num.as_f64() {
                    vm.ctx.new_float(f).into()
                } else {
                    vm.ctx.none()
                }
            }
            JsonValue::String(s) => vm.ctx.new_str(s).into(),
            JsonValue::Array(items) => {
                let mut vals = Vec::new();
                for item in items {
                    vals.push(Self::from_json(vm, item)?);
                }
                vm.ctx.new_list(vals).into()
            }
            JsonValue::Object(items) => {
                let pyd = vm.ctx.new_dict();
                for (k, v) in items {
                    pyd.set_item(k.as_str(), Self::from_json(vm, v)?, vm)?;
                }
                pyd.into()
            }
        })
    }

    /// Convert a Python object into JSON through Python `json.dumps()`
    /// # Arguments
    /// * `vm` - RustPython virtual machine
    /// * `obj` - Python object to serialise
    /// # Returns
    /// * `Result<JsonValue>` - JSON value representation
    fn dumps_json(vm: &VirtualMachine, obj: PyObjectRef) -> Result<JsonValue> {
        let jsonmod = match vm.import("json", 0) {
            Ok(m) => m,
            Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to import json module: {err:?}"))),
        };
        let dumped = match vm.call_method(&jsonmod, "dumps", (obj,)) {
            Ok(v) => v,
            Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to serialise Python value to JSON: {err:?}"))),
        };
        let s = match dumped.downcast::<PyStr>() {
            Ok(s) => s.as_str().to_string(),
            Err(_) => return Err(Py3RuntimeError::Vm("Python json.dumps() did not return a string".to_string())),
        };

        match serde_json::from_str::<JsonValue>(&s) {
            Ok(v) => Ok(v),
            Err(err) => Err(Py3RuntimeError::Vm(format!("Unable to parse Python JSON output: {err}"))),
        }
    }

    /// Resolve Python runtime module name into an absolute file path
    /// # Arguments
    /// * `modname` - Python module name, dotted or plain
    /// # Returns
    /// * `PathBuf` - Absolute module file path
    fn module_path(&self, modname: &str) -> PathBuf {
        self.scripts_dir.join(format!("{}.py", modname.replace('.', "/").trim_matches('/')))
    }

    /// Read Python module source code by runtime module name
    /// # Arguments
    /// * `modname` - Python module name, dotted or plain
    /// # Returns
    /// * `Result<String>` - Python source code
    pub fn read_module_code(&self, modname: &str) -> Result<String> {
        let path = self.module_path(modname);
        match std::fs::read_to_string(&path) {
            Ok(code) => Ok(code),
            Err(err) => Err(Py3RuntimeError::ReadFile { path: path.to_string_lossy().to_string(), source: err }),
        }
    }

    /// Install Python logging bridge prelude into the module scope
    /// # Arguments
    /// * `vm` - RustPython virtual machine
    /// * `scope` - Python execution scope
    /// # Returns
    /// * `Result<()>` - Result of the operation
    fn exec_prelude(&self, vm: &VirtualMachine, scope: rustpython_vm::scope::Scope) -> Result<()> {
        let prelude = r#"
import rtlog as _sysinspect_rtlog

class _SysinspectLogger:
    def error(self, *args):
        _sysinspect_rtlog.write("ERROR", " ".join(map(str, args)))

    def warn(self, *args):
        _sysinspect_rtlog.write("WARN", " ".join(map(str, args)))

    def info(self, *args):
        _sysinspect_rtlog.write("INFO", " ".join(map(str, args)))

    def debug(self, *args):
        _sysinspect_rtlog.write("DEBUG", " ".join(map(str, args)))

log = _SysinspectLogger()
"#;
        let code = match vm.compile(prelude, Exec, "<sysinspect-prelude>".to_string()) {
            Ok(code) => code,
            Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to compile Python prelude: {err}"))),
        };

        match vm.run_code_obj(code, scope) {
            Ok(_) => Ok(()),
            Err(err) => {
                let mut buff = String::new();
                _ = vm.write_exception(&mut buff, &err);
                Err(Py3RuntimeError::Vm(format!("Unable to run Python prelude: {}", buff.trim())))
            }
        }
    }

    /// Execute a Python snippet inside an existing scope
    /// # Arguments
    /// * `vm` - RustPython virtual machine
    /// * `scope` - Python execution scope
    /// * `code` - Python source code snippet
    /// * `filename` - Virtual filename for diagnostics
    /// # Returns
    /// * `Result<()>` - Result of the operation
    fn exec_scope_code(&self, vm: &VirtualMachine, scope: rustpython_vm::scope::Scope, code: &str, filename: &str) -> Result<()> {
        let code_obj = match vm.compile(code, Exec, filename.to_string()) {
            Ok(code) => code,
            Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to compile Python source {filename}: {err}"))),
        };

        match vm.run_code_obj(code_obj, scope) {
            Ok(_) => Ok(()),
            Err(err) => {
                let mut buff = String::new();
                _ = vm.write_exception(&mut buff, &err);
                Err(Py3RuntimeError::Vm(format!("Unable to run Python source {filename}: {}", buff.trim())))
            }
        }
    }

    /// Set active Python module name for both runtime state and log bridge
    /// # Arguments
    /// * `modname` - Python module name
    fn set_modulename(&self, modname: &str) {
        if let Ok(mut m) = self.modulename.lock() {
            *m = modname.to_string();
        }
        set_log_modulename(modname);
    }

    /// Synchronise global Python log buffer into runtime instance state
    /// # Returns
    /// * `Vec<String>` - Buffered log lines
    fn sync_logs(&self) -> Vec<String> {
        let logs = get_log_buffer();
        if let Ok(mut g) = self.logs.lock() {
            *g = logs.clone();
        }
        logs
    }

    /// Compile and execute Python code, then call `run(req)`
    /// # Arguments
    /// * `filename` - Virtual filename for Python compiler diagnostics
    /// * `code` - Python module source
    /// * `modname` - Python module name
    /// * `req` - Runtime request payload
    /// # Returns
    /// * `Result<JsonValue>` - Module result converted to JSON
    fn run_code(&self, filename: &str, code: &str, modname: &str, req: &JsonValue) -> Result<JsonValue> {
        self.itp.enter(|vm| {
            self.load_pylib(vm)?;

            let scope = vm.new_scope_with_builtins();
            self.exec_prelude(vm, scope.clone())?;
            if let Err(err) = scope.globals.set_item("__module_name", vm.ctx.new_str(modname).into(), vm) {
                return Err(Py3RuntimeError::Vm(format!("Unable to set __module_name: {err:?}")));
            }

            let code_obj = match vm.compile(code, Exec, filename.to_string()) {
                Ok(obj) => obj,
                Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to compile source code for {modname}: {err}"))),
            };

            if let Err(err) = vm.run_code_obj(code_obj, scope.clone()) {
                let mut buff = String::new();
                _ = vm.write_exception(&mut buff, &err);
                return Err(Py3RuntimeError::Vm(format!("Error running Python module \"{modname}\": {}", buff.trim())));
            }

            let entrypoint = RuntimeSpec::MainEntryFunction.to_string();
            let runfn = match scope.globals.get_item(&entrypoint, vm) {
                Ok(obj) => obj,
                Err(err) => {
                    let mut buff = String::new();
                    _ = vm.write_exception(&mut buff, &err);
                    return Err(Py3RuntimeError::Vm(format!("Python module must export run(req): {}", buff.trim())));
                }
            };

            let py_req = match Self::from_json(vm, req.clone()) {
                Ok(v) => v,
                Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to convert request to Python object: {err:?}"))),
            };

            let result = match runfn.call(FuncArgs::new(vec![py_req], KwArgs::default()), vm) {
                Ok(v) => v,
                Err(err) => {
                    let mut buff = String::new();
                    _ = vm.write_exception(&mut buff, &err);
                    return Err(Py3RuntimeError::Vm(format!("Error calling run(req) in Python module \"{modname}\": {}", buff.trim())));
                }
            };

            Self::dumps_json(vm, result)
        })
    }

    /// Normalise Python runtime module documentation payload
    /// # Arguments
    /// * `doc` - Python documentation payload converted to JSON
    /// # Returns
    /// * `JsonValue` - Normalised documentation object
    fn normalise_module_doc(mut doc: JsonValue) -> JsonValue {
        let key = RuntimeSpec::DocumentationFunction.to_string();
        loop {
            let Some(obj) = doc.as_object() else {
                break;
            };
            let Some(inner) = obj.get(&key) else {
                break;
            };
            if obj.len() == 1 {
                doc = inner.clone();
                continue;
            }
            break;
        }
        doc
    }

    /// Call a Python module and return runtime payload with data and logs
    /// # Arguments
    /// * `modname` - Python module name
    /// * `code` - Python module source
    /// * `req` - Runtime request payload
    /// * `with_logs` - Include buffered logs in returned payload
    /// # Returns
    /// * `Result<JsonValue>` - Runtime response payload
    pub fn call_module(&self, modname: &str, code: &str, req: &JsonValue, with_logs: bool) -> Result<JsonValue> {
        clear_log_buffer();
        self.set_modulename(modname);
        let data = self.run_code(&format!("{modname}.py"), code, modname, req)?;
        let logs = if with_logs { self.sync_logs() } else { Vec::new() };
        Ok(serde_json::json!({
            RuntimeSpec::DataSectionField.to_string(): data,
            RuntimeSpec::LogsSectionField.to_string(): logs
        }))
    }

    /// Get and validate module documentation from Python code
    /// # Arguments
    /// * `code` - Python module source
    /// # Returns
    /// * `Result<JsonValue>` - Validated documentation object
    pub fn module_doc(&self, code: &str) -> Result<JsonValue> {
        self.itp.enter(|vm| {
            self.load_pylib(vm)?;

            let scope = vm.new_scope_with_builtins();
            self.exec_prelude(vm, scope.clone())?;
            self.exec_scope_code(vm, scope.clone(), code, "<module-doc>")?;
            self.exec_scope_code(
                vm,
                scope.clone(),
                r#"
if "doc" not in globals():
    __sysinspect_doc = {}
elif callable(doc):
    __sysinspect_doc = doc()
else:
    __sysinspect_doc = doc
"#,
                "<module-doc-normalise>",
            )?;

            let doc = match scope.globals.get_item("__sysinspect_doc", vm) {
                Ok(doc) => Self::normalise_module_doc(Self::dumps_json(vm, doc)?),
                Err(_) => serde_json::json!({ RuntimeSpec::DocumentationFunction.to_string(): {} }),
            };

            validate_module_doc(&doc)?;
            Ok(doc)
        })
    }
}
