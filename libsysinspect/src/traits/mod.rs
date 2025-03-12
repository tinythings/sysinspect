pub mod systraits;

use crate::{SysinspectError, cfg::mmconf::MinionConfig};
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use pest::Parser;
use pest_derive::Parser;
use serde_json::Value;
use systraits::SystemTraits;

/// Standard Traits
pub static SYS_ID: &str = "system.id";
pub static SYS_OS_KERNEL: &str = "system.kernel";
pub static SYS_OS_VERSION: &str = "system.os.version";
pub static SYS_OS_NAME: &str = "system.os.name";
pub static SYS_OS_DISTRO: &str = "system.os.distribution";

pub static SYS_NET_HOSTNAME: &str = "system.hostname";
pub static SYS_NET_HOSTNAME_FQDN: &str = "system.hostname.fqdn";
pub static HW_MEM: &str = "hardware.memory";
pub static HW_SWAP: &str = "hardware.swap";
pub static HW_CPU_TOTAL: &str = "hardware.cpu.total";
pub static HW_CPU_BRAND: &str = "hardware.cpu.brand";
pub static HW_CPU_FREQ: &str = "hardware.cpu.frequency";
pub static HW_CPU_VENDOR: &str = "hardware.cpu.vendor";
pub static HW_CPU_CORES: &str = "hardware.cpu.cores";

#[derive(Parser)]
#[grammar = "traits/traits_query.pest"]
struct QueryParser;

/// Parse a very simple traits query. It returns an array of OR arrays, containing AND values.
/// Example:
///
///     "foo and bar or baz"
///
/// This yields to the following structure:
///
///     [[foo, bar], [baz]]
///
/// Each inner array should be treated with AND operator.
pub fn parse_traits_query(input: &str) -> Result<Vec<Vec<String>>, SysinspectError> {
    let pairs = QueryParser::parse(Rule::expression, input)
        .map_err(|err| SysinspectError::ModelDSLError(format!("Invalid query: {err}")))?;

    let mut out = Vec::new();

    for expr in pairs {
        if expr.as_rule() == Rule::expression {
            for group_pair in expr.into_inner() {
                if group_pair.as_rule() == Rule::group {
                    let mut terms = Vec::new();
                    for term_pair in group_pair.into_inner() {
                        if term_pair.as_rule() == Rule::term {
                            terms.push(term_pair.as_str().trim().to_string());
                        }
                    }
                    out.push(terms);
                }
            }
        }
    }

    Ok(out)
}

/// Parse trait query to trait typed (JSON) query
pub fn to_typed_query(qt: Vec<Vec<String>>) -> Result<Vec<Vec<IndexMap<String, Value>>>, SysinspectError> {
    let mut out: Vec<Vec<IndexMap<String, Value>>> = Vec::default();
    for and_op in qt {
        let mut out_op: Vec<IndexMap<String, Value>> = Vec::default();
        for op in and_op {
            let x = op.replace(":", ": ");
            match serde_yaml::from_str::<IndexMap<String, Value>>(&x) {
                Ok(v) => out_op.push(v),
                Err(e) => return Err(SysinspectError::MinionGeneralError(format!("Broken traits query: {e}"))),
            };
        }
        out.push(out_op);
    }
    Ok(out)
}

pub fn matches_traits(qt: Vec<Vec<IndexMap<String, Value>>>, traits: SystemTraits) -> bool {
    let mut or_op_c: Vec<bool> = Vec::default();
    for and_op in qt {
        let mut and_op_c: Vec<bool> = Vec::default();
        for ophm in and_op {
            // op IndexMap has always just one key and one value
            for (opk, opv) in ophm {
                and_op_c.push(traits.get(&opk).map(|x| x.eq(&opv)).unwrap_or(false));
            }
        }
        or_op_c.push(!and_op_c.contains(&false));
    }

    or_op_c.contains(&true)
}

/// System traits instance. Traits are system properties and attributes
/// on which a minion is running.
///
/// P.S. These are not Rust traits. :-)
static _TRAITS: OnceCell<SystemTraits> = OnceCell::new();

/// Returns a copy of initialised traits.
pub fn get_minion_traits(cfg: Option<&MinionConfig>) -> SystemTraits {
    if let Some(cfg) = cfg {
        return _TRAITS.get_or_init(|| SystemTraits::new(cfg.clone())).to_owned();
    }

    _TRAITS.get_or_init(|| SystemTraits::new(MinionConfig::default())).to_owned()
}
