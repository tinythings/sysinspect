use crate::SysinspectError;

#[derive(Debug, Default)]
pub struct ModArgFunction {
    namespace: Vec<String>,
    fid: String,
}

impl ModArgFunction {
    pub fn new(ns: String, fid: String) -> Result<Self, SysinspectError> {
        let namespace = ns.split('.').map(|s| s.to_string()).collect::<Vec<String>>();
        if namespace.len() != 2 {
            return Err(SysinspectError::ModelDSLError(format!("Function {} does not have two fold namespace: {}", fid, ns)));
        }

        Ok(ModArgFunction { namespace, fid })
    }

    /// Get function namespace
    pub fn namespace(&self) -> String {
        format!("{}.{}", &self.namespace[0], &self.namespace[1])
    }

    pub fn ns_parts(&self) -> Result<[&str; 2], SysinspectError> {
        Ok([&self.namespace[0], &self.namespace[1]])
    }

    /// Get function Id
    pub fn fid(&self) -> &str {
        &self.fid
    }
}
