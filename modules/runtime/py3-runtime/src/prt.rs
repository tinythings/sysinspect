use libcommon::SysinspectError;
use libmodcore::{helpers::RuntimePackageKit, rtdocschema::validate_module_doc, rtspec::RuntimeSpec};
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
    cell::RefCell,
    mem,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub(crate) struct PyLoggerState {
    logs: Vec<String>,
    modulename: String,
}

impl PyLoggerState {
    /// Create logger state for tests and runtime initialisation
    /// # Arguments
    /// * `logs` - Initial buffered log lines
    /// * `modulename` - Active module name
    /// # Returns
    /// * `Self` - Logger state instance
    #[cfg(test)]
    pub(crate) fn new(logs: Vec<String>, modulename: String) -> Self {
        Self { logs, modulename }
    }
}

thread_local! {
    static ACTIVE_PY_LOG_STATE: RefCell<Option<Arc<Mutex<PyLoggerState>>>> = const { RefCell::new(None) };
}

struct ActiveLogStateGuard {
    prev: Option<Arc<Mutex<PyLoggerState>>>,
}

impl Drop for ActiveLogStateGuard {
    fn drop(&mut self) {
        ACTIVE_PY_LOG_STATE.with(|slot| {
            *slot.borrow_mut() = self.prev.take();
        });
    }
}

fn set_active_log_state(state: Arc<Mutex<PyLoggerState>>) -> ActiveLogStateGuard {
    let prev = ACTIVE_PY_LOG_STATE.with(|slot| slot.borrow_mut().replace(state));
    ActiveLogStateGuard { prev }
}

/// Set current Python module name for forwarded log messages
/// # Arguments
/// * `modname` - Current Python module name
fn set_log_modulename(state: &Arc<Mutex<PyLoggerState>>, modname: &str) {
    if let Ok(mut g) = state.lock() {
        g.modulename = modname.to_string();
    }
}

/// Clear buffered Python log lines before a module call
pub(crate) fn clear_log_buffer(state: &Arc<Mutex<PyLoggerState>>) {
    if let Ok(mut g) = state.lock() {
        g.logs.clear();
    }
}

/// Take buffered Python log lines and clear the underlying buffer
/// # Returns
/// * `Vec<String>` - Collected log lines
pub(crate) fn take_log_buffer(state: &Arc<Mutex<PyLoggerState>>) -> Vec<String> {
    if let Ok(mut g) = state.lock() { mem::take(&mut g.logs) } else { Vec::new() }
}

/// Push a single formatted log line into the runtime buffer
/// # Arguments
/// * `level` - Log level
/// * `msg` - Log message
fn push_log_line(level: &str, msg: &str) {
    ACTIVE_PY_LOG_STATE.with(|slot| {
        let Some(state) = slot.borrow().clone() else {
            return;
        };

        if let Ok(mut g) = state.lock() {
            let ts = chrono::Local::now().format("%d/%m/%Y %H:%M:%S");
            let modulename = if g.modulename.is_empty() { "Python module".to_string() } else { g.modulename.clone() };
            g.logs.push(format!("[{ts}] - {level}: [{modulename}] {msg}"));
        }
    });
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

#[pymodule]
mod rtpackagekit {
    use super::RuntimePackageKit;
    use rustpython_vm::{PyResult, VirtualMachine};

    /// Return whether PackageKit is reachable on this system.
    /// # Returns
    /// * `PyResult<bool>` - True when PackageKit responds over D-Bus.
    #[pyfunction]
    fn available(_vm: &VirtualMachine) -> PyResult<bool> {
        Ok(RuntimePackageKit::available())
    }

    /// Return current PackageKit daemon status as JSON text.
    /// # Returns
    /// * `PyResult<String>` - JSON object encoded as string.
    #[pyfunction]
    fn status(vm: &VirtualMachine) -> PyResult<String> {
        serde_json::to_string(&RuntimePackageKit::status().map_err(|err| vm.new_runtime_error(err.to_string()))?)
            .map_err(|err| vm.new_runtime_error(err.to_string()))
    }

    /// Return PackageKit history as JSON text.
    /// # Arguments
    /// * `names` - Package names to inspect.
    /// * `count` - Optional history depth.
    /// # Returns
    /// * `PyResult<String>` - JSON array/object encoded as string.
    #[pyfunction]
    fn history(names: Vec<String>, count: Option<u32>, vm: &VirtualMachine) -> PyResult<String> {
        serde_json::to_string(&RuntimePackageKit::history(names, count.unwrap_or(10)).map_err(|err| vm.new_runtime_error(err.to_string()))?)
            .map_err(|err| vm.new_runtime_error(err.to_string()))
    }

    /// Return installed packages snapshot as JSON text.
    /// # Returns
    /// * `PyResult<String>` - JSON array encoded as string.
    #[pyfunction]
    fn packages(vm: &VirtualMachine) -> PyResult<String> {
        serde_json::to_string(&RuntimePackageKit::packages().map_err(|err| vm.new_runtime_error(err.to_string()))?)
            .map_err(|err| vm.new_runtime_error(err.to_string()))
    }

    /// Install packages through PackageKit and return JSON text.
    /// # Arguments
    /// * `names` - Package names to install.
    /// # Returns
    /// * `PyResult<String>` - JSON result encoded as string.
    #[pyfunction]
    fn install(names: Vec<String>, vm: &VirtualMachine) -> PyResult<String> {
        serde_json::to_string(&RuntimePackageKit::install(names).map_err(|err| vm.new_runtime_error(err.to_string()))?)
            .map_err(|err| vm.new_runtime_error(err.to_string()))
    }

    /// Remove packages through PackageKit and return JSON text.
    /// # Arguments
    /// * `names` - Package names to remove.
    /// # Returns
    /// * `PyResult<String>` - JSON result encoded as string.
    #[pyfunction]
    fn remove(names: Vec<String>, vm: &VirtualMachine) -> PyResult<String> {
        serde_json::to_string(&RuntimePackageKit::remove(names).map_err(|err| vm.new_runtime_error(err.to_string()))?)
            .map_err(|err| vm.new_runtime_error(err.to_string()))
    }

    /// Upgrade packages through PackageKit and return JSON text.
    /// # Arguments
    /// * `names` - Package names to upgrade.
    /// # Returns
    /// * `PyResult<String>` - JSON result encoded as string.
    #[pyfunction]
    fn upgrade(names: Vec<String>, vm: &VirtualMachine) -> PyResult<String> {
        serde_json::to_string(&RuntimePackageKit::upgrade(names).map_err(|err| vm.new_runtime_error(err.to_string()))?)
            .map_err(|err| vm.new_runtime_error(err.to_string()))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Py3RuntimeError {
    #[error("python runtime error: {0}")]
    Runtime(#[from] SysinspectError),

    #[error("python vm error: {0}")]
    Vm(String),

    #[error("invalid python module name '{modname}'")]
    InvalidModuleName { modname: String },

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
    log_state: Arc<Mutex<PyLoggerState>>,
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
        let builder = rustpython::Interpreter::builder(Settings::default());
        let rtlog_def = rtlog::module_def(&builder.ctx);
        let rtpackagekit_def = rtpackagekit::module_def(&builder.ctx);
        let itp = builder.init_stdlib().add_native_module(rtlog_def).add_native_module(rtpackagekit_def).build();

        Ok(Self {
            itp,
            scripts_dir,
            lib_dir,
            log_state: Arc::new(Mutex::new(PyLoggerState { logs: Vec::new(), modulename: "Python module".to_string() })),
        })
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
        let syspath = match match vm.import("sys", 0) {
            Ok(m) => m,
            Err(_) => return Err(Py3RuntimeError::Vm("Unable to import sys module".to_string())),
        }
        .get_attr("path", vm)
        {
            Ok(path) => path,
            Err(_) => return Err(Py3RuntimeError::Vm("Unable to access sys.path".to_string())),
        };

        for path in [&self.scripts_dir, &self.lib_dir] {
            let path_str = path.to_string_lossy().to_string();
            let contains = match vm.call_method(&syspath, "__contains__", (path_str.clone(),)) {
                Ok(found) => {
                    found.try_to_bool(vm).map_err(|err| Py3RuntimeError::Vm(format!("Failed to inspect sys.path for {}: {err:?}", path.display())))?
                }
                Err(err) => return Err(Py3RuntimeError::Vm(format!("Failed to inspect sys.path for {}: {err:?}", path.display()))),
            };

            if !contains && let Err(err) = vm.call_method(&syspath, "append", (path_str,)) {
                return Err(Py3RuntimeError::Vm(format!("Failed to append Python path {}: {err:?}", path.display())));
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
        let s = match match vm.call_method(&jsonmod, "dumps", (obj,)) {
            Ok(v) => v,
            Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to serialise Python value to JSON: {err:?}"))),
        }
        .downcast::<PyStr>()
        {
            Ok(s) => s.to_string(),
            Err(_) => return Err(Py3RuntimeError::Vm("Python json.dumps() did not return a string".to_string())),
        };

        match serde_json::from_str::<JsonValue>(&s) {
            Ok(v) => Ok(v),
            Err(err) => Err(Py3RuntimeError::Vm(format!("Unable to parse Python JSON output: {err}"))),
        }
    }

    fn is_valid_module_name(modname: &str) -> bool {
        !modname.is_empty()
            && modname.split('.').all(|segment| !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
    }

    /// Resolve Python runtime module name into an absolute file path
    /// # Arguments
    /// * `modname` - Python module name, dotted or plain
    /// # Returns
    /// * `Result<PathBuf>` - Absolute module file path
    fn module_path(&self, modname: &str) -> Result<PathBuf> {
        if !Self::is_valid_module_name(modname) {
            return Err(Py3RuntimeError::InvalidModuleName { modname: modname.to_string() });
        }

        Ok(self.scripts_dir.join(format!("{}.py", modname.replace('.', "/"))))
    }

    /// Read Python module source code by runtime module name
    /// # Arguments
    /// * `modname` - Python module name, dotted or plain
    /// # Returns
    /// * `Result<String>` - Python source code
    pub fn read_module_code(&self, modname: &str) -> Result<String> {
        let path = self.module_path(modname)?;
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
import rtpackagekit as _sysinspect_rtpackagekit
import json as _sysinspect_json

_sysinspect_req = {}

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

class _SysinspectPackageKit:
    def available(self):
        return _sysinspect_rtpackagekit.available()

    def status(self):
        return _sysinspect_json.loads(_sysinspect_rtpackagekit.status())

    def history(self, names, count=10):
        return _sysinspect_json.loads(_sysinspect_rtpackagekit.history(names, count))

    def packages(self):
        return _sysinspect_json.loads(_sysinspect_rtpackagekit.packages())

    def install(self, names):
        return _sysinspect_json.loads(_sysinspect_rtpackagekit.install(names))

    def remove(self, names):
        return _sysinspect_json.loads(_sysinspect_rtpackagekit.remove(names))

    def upgrade(self, names):
        return _sysinspect_json.loads(_sysinspect_rtpackagekit.upgrade(names))

packagekit = _SysinspectPackageKit()

class _SysinspectHost:
    def trait(self, name):
        return _sysinspect_req.get("host", {}).get("traits", {}).get(name)

    def has(self, name):
        return name in _sysinspect_req.get("host", {}).get("traits", {})

    def paths(self):
        return _sysinspect_req.get("host", {}).get("paths", {})

    def path(self, name):
        return _sysinspect_req.get("host", {}).get("paths", {}).get(name)

host = _SysinspectHost()
"#;
        match vm.run_code_obj(
            match vm.compile(prelude, Exec, "<sysinspect-prelude>".to_string()) {
                Ok(code) => code,
                Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to compile Python prelude: {err}"))),
            },
            scope,
        ) {
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
        match vm.run_code_obj(
            match vm.compile(code, Exec, filename.to_string()) {
                Ok(code) => code,
                Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to compile Python source {filename}: {err}"))),
            },
            scope,
        ) {
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
        set_log_modulename(&self.log_state, modname);
    }

    /// Take buffered Python log lines from the runtime state
    /// # Returns
    /// * `Vec<String>` - Buffered log lines
    fn take_logs(&self) -> Vec<String> {
        take_log_buffer(&self.log_state)
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

            if let Err(err) = vm.run_code_obj(
                match vm.compile(code, Exec, filename.to_string()) {
                    Ok(obj) => obj,
                    Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to compile source code for {modname}: {err}"))),
                },
                scope.clone(),
            ) {
                let mut buff = String::new();
                _ = vm.write_exception(&mut buff, &err);
                return Err(Py3RuntimeError::Vm(format!("Error running Python module \"{modname}\": {}", buff.trim())));
            }

            let runfn = match scope.globals.get_item(&RuntimeSpec::MainEntryFunction.to_string(), vm) {
                Ok(obj) => obj,
                Err(err) => {
                    let mut buff = String::new();
                    _ = vm.write_exception(&mut buff, &err);
                    return Err(Py3RuntimeError::Vm(format!("Python module must export run(req): {}", buff.trim())));
                }
            };

            let req_obj = match Self::from_json(vm, req.clone()) {
                Ok(v) => v,
                Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to convert request to Python object: {err:?}"))),
            };
            if let Err(err) = scope.globals.set_item("_sysinspect_req", req_obj.clone(), vm) {
                return Err(Py3RuntimeError::Vm(format!("Unable to set _sysinspect_req: {err:?}")));
            }

            Self::dumps_json(
                vm,
                match runfn.call(FuncArgs::new(vec![req_obj], KwArgs::default()), vm) {
                    Ok(v) => v,
                    Err(err) => {
                        let mut buff = String::new();
                        _ = vm.write_exception(&mut buff, &err);
                        return Err(Py3RuntimeError::Vm(format!("Error calling run(req) in Python module \"{modname}\": {}", buff.trim())));
                    }
                },
            )
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
        clear_log_buffer(&self.log_state);
        self.set_modulename(modname);
        let _active_log_state = set_active_log_state(self.log_state.clone());
        Ok(serde_json::json!({
            RuntimeSpec::DataSectionField.to_string(): self.run_code(&format!("{modname}.py"), code, modname, req)?,
            RuntimeSpec::LogsSectionField.to_string(): if with_logs { self.take_logs() } else { take_log_buffer(&self.log_state) }
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
            let empty_req = match Self::from_json(vm, serde_json::json!({})) {
                Ok(req) => req,
                Err(err) => return Err(Py3RuntimeError::Vm(format!("Unable to convert empty request to Python object: {err:?}"))),
            };
            if let Err(err) = scope.globals.set_item("_sysinspect_req", empty_req, vm) {
                return Err(Py3RuntimeError::Vm(format!("Unable to set empty _sysinspect_req for docs: {err:?}")));
            }
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
