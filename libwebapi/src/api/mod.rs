use actix_web::Scope;

pub use v1::V1;
pub mod v1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiVersions {
    V1,
}

/// Each version should implement this trait to provide its own API handlers.
pub trait ApiVersion {
    fn load(&self, scope: Scope) -> Scope;
}

/// Get the API version implementation based on the requested version.
pub fn get(dev_mode: bool, port: u16, version: ApiVersions) -> Option<Box<dyn ApiVersion>> {
    match version {
        ApiVersions::V1 => Some(Box::new(V1::new(dev_mode, port))),
    }
}
