use byte_unit::Byte;
use libsysinspect::SysinspectError;
use regex::Regex;
use serde_json::Value;

#[derive(Debug, PartialEq)]
enum ExprValue {
    Integer(i64),
    Float(f64),
    Bytes(u128),
    Text(String),
}

#[derive(Debug, PartialEq)]
enum Op {
    Greater,      // >
    GreaterEqual, // >=
    Less,         // <
    LessEqual,    // <=
    NotEqual,     // !
    Equal,        // =
    Unknown,
}

/// ExpressionParser is a simple expression parser that can evaluate expressions
/// against a value. It supports comparison operators (>, <, =, !=) and glob patterns (*).
/// It is not a full-featured expression parser, but it can handle simple cases.
/// It is designed to be used with telemetry data, where the expressions are used to filter
/// or transform the data before it is sent to the telemetry backend.
///
/// The parser is not thread-safe and should be used in a single-threaded context.
/// It is not designed to be used in a multi-threaded context.
///
/// # Syntax
///
/// The syntax for the expressions is as follows:
/// <operator><value>
/// Where <value> is a data value, <operator> is one of the comparison operators
/// (>, <, =, !=) or a glob pattern (*).
///
/// # Examples
/// ```
///     >=2GB
///     <100
///     >2GB
///     *.foo.com
/// ```
///
/// Supported operators:
/// ```
///     >, <, =, !=
/// ```
///
/// Supported UNIX glob patterns:
/// ```
///     * (matches any string)
///     ? (matches a single character)
///     + (matches one or more characters)
/// ```
struct ExpressionParser {
    expr: String,
    value: Value,
}

impl ExpressionParser {
    fn new(expr: String) -> Self {
        ExpressionParser { expr, value: Value::Null }
    }

    fn is_expression(&self) -> bool {
        self.expr.contains('>') || self.expr.contains('<') || self.expr.contains('=') || self.expr.contains('!')
    }

    fn is_glob(&self) -> bool {
        self.expr.contains('*') || self.expr.contains('?') || self.expr.contains('+')
    }

    fn get_expr(input: &str) -> Result<(Op, String), SysinspectError> {
        let re = Regex::new(
            r"(?xi)
        ^\s*
        (?P<op>
            >  |   # greater
            >= |   # greater equal
            <  |   # less
            <= |   # less equal
            !  |   # not
            =      # equal
        )
        \s*
        (?P<size>\d+(\.\d+)?\s*[KMGTPE]?i?B)
        \s*$
    ",
        )
        .map_err(|e| SysinspectError::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))?;

        let caps = re.captures(input).ok_or_else(|| {
            SysinspectError::from(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("invalid expression: {:?}", input),
            )) as Box<dyn std::error::Error + Send + Sync>)
        })?;

        Ok((
            match &caps["op"] {
                ">" => Op::Greater,
                ">=" => Op::GreaterEqual,
                "<" => Op::Less,
                "<=" => Op::LessEqual,
                "!" => Op::NotEqual,
                "=" => Op::Equal,
                _ => Op::Unknown,
            },
            caps["size"].trim().to_string(),
        ))
    }

    fn get_value(s: &str) -> ExprValue {
        if let Ok(i) = s.parse::<i64>() {
            return ExprValue::Integer(i);
        }

        if let Ok(f) = s.parse::<f64>() {
            return ExprValue::Float(f);
        }

        if let Ok(b) = Byte::parse_str(s, false) {
            return ExprValue::Bytes(b.into());
        }
        ExprValue::Text(s.to_string())
    }

    fn cmp_expr(&self) -> bool {
        log::debug!("Comparing expression: {}", self.expr);
        let (op, size) = match Self::get_expr(&self.expr) {
            Ok((op, size)) => (op, size),
            Err(e) => {
                log::error!("Error parsing expression: {}", e);
                return false;
            }
        };

        match Self::get_value(&size) {
            ExprValue::Integer(i) => {
                if let Some(v) = self.value.as_i64() {
                    return match op {
                        Op::Greater => v > i,
                        Op::GreaterEqual => v >= i,
                        Op::Less => v < i,
                        Op::LessEqual => v <= i,
                        Op::NotEqual => v != i,
                        Op::Equal => v == i,
                        _ => false,
                    };
                }
            }
            ExprValue::Float(f) => {
                if let Some(v) = self.value.as_f64() {
                    return match op {
                        Op::Greater => v > f,
                        Op::GreaterEqual => v >= f,
                        Op::Less => v < f,
                        Op::LessEqual => v <= f,
                        Op::NotEqual => v != f,
                        Op::Equal => v == f,
                        _ => false,
                    };
                }
            }
            ExprValue::Bytes(b) => {
                if let Some(v) = self.value.as_u64() {
                    return match op {
                        Op::Greater => v > b as u64,
                        Op::GreaterEqual => v >= b as u64,
                        Op::Less => v < b as u64,
                        Op::LessEqual => v <= b as u64,
                        Op::NotEqual => v != b as u64,
                        Op::Equal => v == b as u64,
                        _ => false,
                    };
                }
            }
            ExprValue::Text(s) => {
                if let Some(v) = self.value.as_str() {
                    return match op {
                        Op::NotEqual => s != *v,
                        Op::Equal => s == *v,
                        _ => false,
                    };
                }
            }
        }
        log::debug!("Error comparing expression: {} with value: {}", self.expr, self.value);
        false
    }

    fn cmp_direct(&self) -> bool {
        log::debug!("Direct comparison: {}", self.expr);
        let val_str = self.value.as_str().unwrap_or_default();
        let expr_str = self.expr.as_str();

        if val_str == expr_str {
            return true;
        }

        false
    }

    fn glob(&self) -> bool {
        log::debug!("Glob expression: {}", self.expr);
        false
    }

    fn eval(&mut self, val: Value) -> bool {
        self.value = val;
        if self.is_expression() {
            return self.cmp_expr();
        } else if self.is_glob() {
            return self.glob();
        }
        self.cmp_direct()
    }
}

/// Parse the expression and return a boolean value.
/// Example:
///
/// expr(">2GB", 4294967296)
pub fn expr(expr: &str, val: Value) -> bool {
    ExpressionParser::new(expr.to_string()).eval(val)
}
