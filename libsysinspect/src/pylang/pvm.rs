/*
Python virtual machine
 */

use crate::SysinspectError;
use colored::Colorize;
use rustpython_vm::{
    builtins::PyStr,
    compiler::Mode::Exec,
    convert::IntoObject,
    function::{FuncArgs, KwArgs},
    AsObject, PyRef, PyResult,
};
use rustpython_vm::{Interpreter, Settings};
use rustpython_vm::{PyObjectRef, VirtualMachine};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct PyVm {
    itp: Interpreter,
    libpath: String,
    modpath: String,
}

impl PyVm {
    pub fn new(libpath: Option<String>, modpath: Option<String>) -> Self {
        let mut cfg = Settings::default();
        let libpath = libpath.unwrap_or("/usr/share/sysinspect/lib".to_string());
        cfg.path_list.push(libpath.to_string());

        let itp = rustpython::InterpreterConfig::new()
            .init_stdlib()
            .settings(cfg)
            /*
            .init_hook(Box::new(|vm| {
                vm.add_native_module("rust_py_module".to_owned(), Box::new(rust_py_module::make_module));
            }))
            */
            .interpreter();

        Self { itp, libpath: libpath.to_string(), modpath: modpath.unwrap_or("/usr/share/sysinspect/modules/".to_string()) }
    }

    /// Load main script of a module by a regular namespace
    fn load_script(&self, ns: &str) -> Result<String, SysinspectError> {
        // XXX: util::get_namespace() ? Because something similar exists for the binaries
        let pbuff = PathBuf::from(&self.modpath)
            .join(format!("{}.py", ns.replace(".", "/").trim_start_matches("/").trim_end_matches("/")));
        if pbuff.exists() {
            return Ok(fs::read_to_string(pbuff).unwrap_or_default());
        }

        Err(SysinspectError::ModuleError(format!("Module at {} was not found", pbuff.to_str().unwrap_or_default().yellow())))
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
    pub fn call(
        self: Arc<&Self>, namespace: &str, opts: Option<Vec<Value>>, args: Option<HashMap<String, Value>>,
    ) -> Result<(), SysinspectError> {
        self.itp.enter(|vm| {
            let lpth = Path::new(&self.libpath);
            if !lpth.exists() || !lpth.is_dir() {
                return Err(SysinspectError::ModuleError(format!("Directory {} does not exist", &self.libpath)));
            }
            self.load_pylib(vm)?;

            let code_obj = match vm.compile(&self.load_script(namespace)?, Exec, "<embedded>".to_owned()) {
                Ok(src) => src,
                Err(err) => {
                    return Err(SysinspectError::ModuleError(format!("Unable to compile source code for {namespace}: {err}")));
                }
            };

            log::debug!("Prepared to launch dispatcher for python script");

            let scope = vm.new_scope_with_builtins();
            if let Err(err) = vm.run_code_obj(code_obj, scope.clone()) {
                return Err(SysinspectError::ModuleError(format!("Error running \"{namespace}\" Python module: {:?}", err)));
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
            let farg = FuncArgs::new(py_opts, kwargs);

            let dispatcher_function = scope.globals.get_item("dispatch", vm).expect("Failed to find `dispatch` function");
            let result = match dispatcher_function.call(farg, vm) {
                Ok(r) => r,
                Err(err) => {
                    vm.print_exception(err);
                    return Ok(());
                }
            };

            if let Ok(py_str) = result.downcast::<rustpython_vm::builtins::PyStr>() {
                println!("{}", py_str.as_str());
            } else {
                println!("error: no string return");
            }
            Ok(())
        })
    }
}
