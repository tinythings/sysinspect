pub mod systraits;

use crate::SysinspectError;
use pest::Parser;
use pest_derive::Parser;

/// Standard Traits
pub static SYS_ID: &str = "system.id";
pub static SYS_OS_KERNEL: &str = "system.kernel";
pub static SYS_OS_VERSION: &str = "system.os.version";
pub static SYS_OS_NAME: &str = "system.os.name";
pub static SYS_OS_DISTRO: &str = "system.os.distribution";

pub static SYS_NET_HOSTNAME: &str = "system.hostname";
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
pub fn get_traits_query(input: &str) -> Result<Vec<Vec<String>>, SysinspectError> {
    let mut out = Vec::new();
    let mut pairs = match QueryParser::parse(Rule::expression, input) {
        Ok(prs) => prs,
        Err(err) => return Err(SysinspectError::ModelDSLError(format!("Invalid query: {err}"))),
    };

    let expr = match pairs.next() {
        Some(expr) => expr,
        None => return Ok(out),
    };

    for grp in expr.into_inner() {
        if grp.as_rule() == Rule::group {
            let mut terms = Vec::new();
            for t_pair in grp.into_inner() {
                if t_pair.as_rule() == Rule::term {
                    terms.push(t_pair.as_str().to_string());
                }
            }
            out.push(terms);
        }
    }

    Ok(out)
}
