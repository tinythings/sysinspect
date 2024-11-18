pub mod datapatch;
pub mod distr;
pub mod mspec;
pub mod mspecdef;

/// DSL directives
pub static DSL_DIR_ENTITIES: &str = "entities";
pub static DSL_DIR_ACTIONS: &str = "actions";
pub static DSL_DIR_RELATIONS: &str = "relations";
pub static DSL_DIR_CONSTRAINTS: &str = "constraints";

// Config and index
pub static DSL_IDX_CHECKBOOK: &str = "checkbook";
pub static DSL_IDX_CFG: &str = "config";

// This one belongs to the configuration, but is defined
// outside in the tree, as there migt be really many of those.
pub static DSL_IDX_EVENTS_CFG: &str = "events";
