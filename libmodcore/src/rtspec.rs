use strum_macros::{Display, EnumString};

#[derive(Debug, Display, EnumString)]
pub enum RuntimeParams {
    #[strum(serialize = "rt:mod")]
    ModuleName,
}

#[derive(Debug, Display, EnumString)]
pub enum RuntimeSpec {
    #[strum(serialize = "run")]
    MainEntryFunction,

    #[strum(serialize = "doc")]
    DocumentationFunction,
}

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

pub struct RuntimeModuleDocPrefix {
    pub prefix: String,
}

impl RuntimeModuleDocPrefix {
    pub fn new(prefix: &RuntimeSpec) -> Self {
        Self { prefix: prefix.to_string() }
    }

    pub fn field(&self, field: &RuntimeModuleDocumentation) -> String {
        format!("{}.{}", self.prefix, field)
    }
}
