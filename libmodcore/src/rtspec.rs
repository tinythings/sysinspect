use strum_macros::{Display, EnumString};

/// RuntimeParams enum
/// Holds runtime parameter names
#[derive(Debug, Display, EnumString)]
pub enum RuntimeParams {
    #[strum(serialize = "rt.mod")]
    ModuleName,

    #[strum(serialize = "rt.man")]
    ModuleManual,

    #[strum(serialize = "rt.")]
    RtPrefix,
}

/// RuntimeSpec enum
/// Holds runtime specification field names
#[derive(Debug, Display, EnumString)]
pub enum RuntimeSpec {
    #[strum(serialize = "run")]
    MainEntryFunction,

    #[strum(serialize = "doc")]
    DocumentationFunction,

    /// Logs section field for returning logs from module
    /// within the returned data object.
    #[strum(serialize = "__sysinspect-module-logs")]
    LogsSectionField,

    #[strum(serialize = "data")]
    DataSectionField,
}

/// RuntimeModuleDocumentation enum
/// Holds documentation field names
#[derive(Debug, Display, EnumString)]
pub enum RuntimeModuleDocumentation {
    #[strum(serialize = "name")]
    Name,

    #[strum(serialize = "version")]
    Version,

    #[strum(serialize = "description")]
    Description,

    #[strum(serialize = "author")]
    Author,

    #[strum(serialize = "license")]
    License,

    #[strum(serialize = "arguments")]
    Arguments,

    #[strum(serialize = "options")]
    Options,

    #[strum(serialize = "examples")]
    Examples,

    #[strum(serialize = "returns")]
    Returns,

    #[strum(serialize = "code")]
    Code,

    #[strum(serialize = "required")]
    Required,

    #[strum(serialize = "type")]
    Type,
}

/// RuntimeModuleDocPrefix struct
/// Holds prefix for runtime module documentation fields
/// # Fields
/// * `prefix` - Prefix string
pub struct RuntimeModuleDocPrefix {
    pub prefix: String,
}

/// RuntimeModuleDocPrefix implementation
impl RuntimeModuleDocPrefix {
    /// Create new RuntimeModuleDocPrefix
    /// # Arguments
    /// * `prefix` - RuntimeSpec enumeration
    /// # Returns
    /// RuntimeModuleDocPrefix instance
    pub fn new(prefix: &RuntimeSpec) -> Self {
        Self { prefix: prefix.to_string() }
    }

    /// Get full field name with prefix
    /// # Arguments
    /// * `field` - Field enumeration
    /// # Returns
    /// Full field name string
    pub fn field(&self, field: &RuntimeModuleDocumentation) -> String {
        format!("{}.{}", self.prefix, field)
    }
}

/// Get runtime modules path
/// # Arguments
/// * `runtime_id` - Runtime identifier
/// * `sharelib` - Optional sharelib base path (without `$PATH/lib` part)
/// # Returns
/// Runtime modules path
pub fn runtime_modules_path(runtime_id: &str, sharelib: Option<&str>) -> String {
    format!("/{}/lib/{runtime_id}", sharelib.unwrap_or("/usr/share/sysinspect"))
}
