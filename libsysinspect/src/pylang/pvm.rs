/*
Python virtual machine
 */

use crate::SysinspectError;
use rustpython_vm::compiler::Mode::Exec;
use rustpython_vm::VirtualMachine;
use rustpython_vm::{Interpreter, Settings};
use std::{
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
    fn load_script(&self, ns: &str) -> String {
        // XXX: empty string is also python code!
        fs::read_to_string(PathBuf::from(&self.modpath).join(format!("{ns}.py"))).unwrap_or_default()
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

    pub fn as_ptr(&self) -> Arc<&Self> {
        Arc::new(self)
    }

    /// Call a light Python module
    pub fn call(self: Arc<&Self>, namespace: &str) -> Result<(), SysinspectError> {
        self.itp.enter(|vm| {
            let lpth = Path::new(&self.libpath);
            if !lpth.exists() || !lpth.is_dir() {
                return Err(SysinspectError::ModuleError(format!("Directory {} does not exist", &self.libpath)));
            }
            self.load_pylib(vm)?;

            let code_obj = match vm.compile(&self.load_script(namespace), Exec, "<embedded>".to_owned()) {
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

            let dispatcher_function = scope.globals.get_item("dispatch", vm).expect("Failed to find `dispatch` function");
            let result = match dispatcher_function.call((), vm) {
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
