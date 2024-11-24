/*
Python module, exported to the Python realm
 */

use rustpython_vm::pymodule;

#[pymodule]
pub mod syscore {
    use crate::{cfg::mmconf::MinionConfig, traits::systraits::SystemTraits, util::dataconv};
    use rustpython_vm::PyResult;
    use rustpython_vm::{builtins::PyList, convert::ToPyObject, pyclass, PyObjectRef, PyPayload, VirtualMachine};
    use std::path::PathBuf;

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
            // This sucks big time. It always initialises traits for every script call
            let cfg = match MinionConfig::new(PathBuf::from("/etc/sysinspect/sysinspect.conf")) {
                Ok(c) => Some(c),
                Err(err) => {
                    log::debug!("Error initialising traits: {err}");
                    None
                }
            };

            if let Some(cfg) = cfg {
                return MinionTraits { traits: Some(SystemTraits::new(cfg)) };
            }

            MinionTraits { ..Default::default() }
        }

        #[pymethod]
        fn get(&self, key: String) -> String {
            if self.traits.is_some() {
                return dataconv::to_string(self.traits.clone().and_then(|v| v.get(&key))).unwrap_or_default();
            }
            "".to_string()
        }

        #[pymethod]
        fn list(&self) -> StrVec {
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
}
