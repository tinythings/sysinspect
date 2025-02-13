/*
Python virtual machine
 */

use crate::{pylang::PY_MAIN_FUNC, SysinspectError};
use colored::Colorize;
use indexmap::IndexMap;
use rustpython_vm::{
    compiler::Mode::Exec,
    function::{FuncArgs, KwArgs},
    AsObject, PyResult,
};
use rustpython_vm::{Interpreter, Settings};
use rustpython_vm::{PyObjectRef, VirtualMachine};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::pylib::pysystem::syscore;

pub struct PyVm {
    itp: Interpreter,
    libpath: String,
    modpath: String,
}

impl PyVm {
    pub fn new(libpath: PathBuf, modpath: PathBuf) -> Self {
        let mut cfg = Settings::default();
        let libpath = libpath.to_str().unwrap_or_default();
        cfg.path_list.push(libpath.to_string());

        let itp = rustpython::InterpreterConfig::new()
            .init_stdlib()
            .settings(cfg)
            .init_hook(Box::new(|vm| {
                vm.add_native_module("syscore".to_owned(), Box::new(syscore::make_module));
            }))
            .interpreter();

        Self { itp, libpath: libpath.to_string(), modpath: modpath.to_str().unwrap_or_default().to_string() }
    }

    /// Load main script of a module by a regular namespace
    fn load_by_ns(&self, ns: &str) -> Result<String, SysinspectError> {
        // XXX: util::get_namespace() ? Because something similar exists for the binaries
        let pbuff = PathBuf::from(&self.modpath)
            .join(format!("{}.py", ns.replace(".", "/").trim_start_matches("/").trim_end_matches("/")));
        self.load_by_path(&pbuff)
    }

    fn load_by_path(&self, pth: &PathBuf) -> Result<String, SysinspectError> {
        if pth.exists() {
            return Ok(fs::read_to_string(pth).unwrap_or_default());
        }

        Err(SysinspectError::ModuleError(format!("Module at {} was not found", pth.to_str().unwrap_or_default().yellow())))
    }

    fn load_pylib(&self, vm: &VirtualMachine) -> Result<(), SysinspectError> {
        match vm.import("sys", 0) {
            Ok(sysmod) => match sysmod.get_attr("path", vm) {
                Ok(syspath) => {
                    if let Err(err) = vm.call_method(&syspath, "append", (&self.libpath,)) {
                        return Err(SysinspectError::ModuleError(format!("{:?}", err)));
                    }
                }
                Err(err) => {
                    return Err(SysinspectError::ModuleError(format!("{:?}", err)));
                }
            },
            Err(err) => {
                return Err(SysinspectError::ModuleError(format!("{:?}", err)));
            }
        };
        Ok(())
    }

    #[allow(clippy::arc_with_non_send_sync)]
    pub fn as_ptr(&self) -> Arc<&Self> {
        Arc::new(self)
    }

    #[allow(clippy::wrong_self_convention)]
    #[allow(clippy::only_used_in_recursion)]
    fn from_json(&self, vm: &VirtualMachine, value: Value) -> PyResult<PyObjectRef> {
        Ok(match value {
            Value::Null => vm.ctx.none(),
            Value::Bool(b) => vm.ctx.new_bool(b).into(),
            Value::Number(num) => {
                if let Some(i) = num.as_i64() {
                    vm.ctx.new_int(i).into()
                } else if let Some(f) = num.as_f64() {
                    vm.ctx.new_float(f).into()
                } else {
                    vm.ctx.none()
                }
            }
            Value::String(s) => vm.ctx.new_str(s).into(),
            Value::Array(arr) => vm
                .ctx
                .new_list(arr.into_iter().map(|item| self.from_json(vm, item).expect("Failed to convert JSON")).collect())
                .into(),
            Value::Object(obj) => {
                let py_dict = vm.ctx.new_dict();
                for (key, val) in obj {
                    let py_val = self.from_json(vm, val)?;
                    py_dict.set_item(key.as_str(), py_val, vm)?;
                }
                py_dict.into()
            }
        })
    }

    /// Call a light Python module
    pub fn call<T: AsRef<Path>>(
        self: Arc<&Self>, namespace: T, opts: Option<Vec<Value>>, args: Option<IndexMap<String, Value>>,
    ) -> Result<String, SysinspectError> {
        self.itp.enter(|vm| {
            let lpth = Path::new(&self.libpath);
            if !lpth.exists() || !lpth.is_dir() {
                return Err(SysinspectError::ModuleError(format!("Directory {} does not exist", &self.libpath)));
            }
            self.load_pylib(vm)?;

            // Get script source
            let src = if namespace.as_ref().is_absolute() {
                self.load_by_path(&namespace.as_ref().to_path_buf())?
            } else {
                self.load_by_ns(namespace.as_ref().to_str().unwrap_or_default())?
            };

            let code_obj = match vm.compile(&src, Exec, "<embedded>".to_owned()) {
                Ok(src) => src,
                Err(err) => {
                    return Err(SysinspectError::ModuleError(format!(
                        "Unable to compile source code for {}: {err}",
                        namespace.as_ref().to_str().unwrap_or_default()
                    )));
                }
            };

            log::debug!("Prepared to launch dispatcher for python script");

            let scope = vm.new_scope_with_builtins();
            if let Err(err) = vm.run_code_obj(code_obj, scope.clone()) {
                let mut buff = String::new();
                _ = vm.write_exception(&mut buff, &err);
                return Err(SysinspectError::ModuleError(format!(
                    "Error running Python function \"{}\": {}",
                    namespace.as_ref().to_str().unwrap_or_default(),
                    buff.trim()
                )));
            }

            // opts/args
            let py_opts = opts.into_iter().map(|val| self.from_json(vm, json!(val)).unwrap()).collect::<Vec<_>>();
            let py_args = vm.ctx.new_dict();
            for (key, val) in args.unwrap_or_default() {
                let py_key = vm.ctx.new_str(key);
                let py_val = self.from_json(vm, val).unwrap();
                py_args.set_item(py_key.as_object(), py_val, vm).unwrap();
            }

            let kwargs: KwArgs = py_args
                .into_iter()
                .map(|(k, v)| (k.downcast::<rustpython_vm::builtins::PyStr>().unwrap().as_str().to_string(), v))
                .collect();

            let fref = match scope.globals.get_item(PY_MAIN_FUNC, vm) {
                Ok(fref) => fref,
                Err(err) => {
                    let mut buff = String::new();
                    _ = vm.write_exception(&mut buff, &err);
                    return Err(SysinspectError::ModuleError(format!(
                        "Error running Python function \"{}\": {}",
                        namespace.as_ref().to_str().unwrap_or_default(),
                        buff.trim()
                    )));
                }
            };

            let r = match fref.call(FuncArgs::new(py_opts, kwargs), vm) {
                Ok(r) => r,
                Err(err) => {
                    let mut buff = String::new();
                    _ = vm.write_exception(&mut buff, &err);
                    return Err(SysinspectError::ModuleError(format!(
                        "Error running \"{}\" Python module:\n{}",
                        namespace.as_ref().to_str().unwrap_or_default(),
                        buff.trim()
                    )));
                }
            };

            if let Ok(py_str) = r.downcast::<rustpython_vm::builtins::PyStr>() {
                return Ok(py_str.as_str().to_string());
            }

            Err(SysinspectError::ModuleError("Python script does not returns a JSON string".to_string()))
        })
    }
}
