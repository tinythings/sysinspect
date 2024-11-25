/*
Python module, exported to the Python realm
 */

use rustpython_vm::pymodule;

#[pymodule]
pub mod syscore {
    use crate::{
        traits::{self, systraits::SystemTraits},
        util::dataconv,
    };
    use rustpython_vm::{
        builtins::{PyDict, PyList},
        common::lock::PyMutex,
        convert::ToPyObject,
        pyclass, PyObjectRef, PyPayload, PyRef, VirtualMachine,
    };
    use rustpython_vm::{AsObject, PyResult};
    use std::collections::HashMap;

    #[derive(Debug, Clone)]
    struct StrVec(Vec<String>);
    impl ToPyObject for StrVec {
        fn to_pyobject(self, vm: &VirtualMachine) -> PyObjectRef {
            let l = self.0.into_iter().map(|s| vm.new_pyobj(s)).collect();
            PyList::new_ref(l, vm.as_ref()).to_pyobject(vm)
        }
    }

    #[pyattr]
    #[pyclass(module = "syscore", name = "__MinionTraits")]
    #[derive(Debug, PyPayload, Default)]
    pub struct MinionTraits {
        traits: Option<SystemTraits>,
    }

    #[pyclass]
    impl MinionTraits {
        fn new() -> MinionTraits {
            MinionTraits { traits: Some(traits::get_minion_traits(None)) }
        }

        #[pymethod]
        fn get(&self, key: String, _vm: &VirtualMachine) -> PyObjectRef {
            if self.traits.is_some() {
                return dataconv::to_pyobjectref(self.traits.clone().and_then(|v| v.get(&key)), _vm).unwrap();
            }
            _vm.ctx.none()
        }

        #[pymethod]
        fn list(&self, _vm: &VirtualMachine) -> StrVec {
            let mut out: StrVec = StrVec(vec![]);
            if let Some(traits) = self.traits.clone() {
                for item in traits.items() {
                    out.0.push(item);
                }
            }

            out
        }
    }

    #[pyfunction]
    #[allow(non_snake_case)]
    /// This is a mimic of "MinionTraits" Python class,
    /// which needs to be called for the init.
    fn MinionTraits() -> PyResult<MinionTraits> {
        Ok(MinionTraits::new())
    }

    #[derive(Debug)]
    struct Inner {
        retcode: usize,
        warnings: Vec<String>,
        message: String,
        data: PyRef<PyDict>,
    }

    #[pyattr]
    #[pyclass(module = "syscore", name = "__SysinspectReturn")]
    #[derive(Debug, PyPayload)]
    pub struct SysinspectReturn {
        inner: PyMutex<Inner>,
    }

    #[pyclass]
    impl SysinspectReturn {
        fn new(_vm: &VirtualMachine) -> SysinspectReturn {
            SysinspectReturn {
                inner: PyMutex::new(Inner { retcode: 0, data: _vm.ctx.new_dict(), warnings: vec![], message: "".to_string() }),
            }
        }

        #[pygetset]
        fn retcode(&self) -> usize {
            self.inner.lock().retcode
        }

        #[pymethod]
        fn set_retcode(&self, retcode: usize, _vm: &VirtualMachine) -> PyObjectRef {
            self.inner.lock().retcode = retcode;
            _vm.ctx.none()
        }

        #[pygetset]
        fn message(&self) -> String {
            self.inner.lock().message.to_owned()
        }

        #[pymethod]
        fn set_message(&self, message: String) {
            self.inner.lock().message = message
        }

        #[pygetset]
        fn warnings(&self, _vm: &VirtualMachine) -> PyObjectRef {
            let list = self.inner.lock().warnings.iter().map(|e| _vm.new_pyobj(e)).collect();
            PyList::new_ref(list, _vm.as_ref()).to_pyobject(_vm)
        }

        #[pymethod]
        fn add_warning(&self, warn: String, _vm: &VirtualMachine) {
            self.inner.lock().warnings.push(warn);
        }

        #[pygetset]
        fn data(&self) -> PyRef<PyDict> {
            self.inner.lock().data.to_owned()
        }

        #[pymethod]
        fn set_data(&self, data: PyRef<PyDict>) {
            self.inner.lock().data = data;
        }
    }

    #[pyfunction]
    #[allow(non_snake_case)]
    /// This is a mimic of "MinionTraits" Python class,
    /// which needs to be called for the init.
    fn SysinspectReturn(_vm: &VirtualMachine) -> PyResult<SysinspectReturn> {
        Ok(SysinspectReturn::new(_vm))
    }
}
