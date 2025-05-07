use serde_json::Value;

struct ExpressionParser {
    expr: String,
    value: Value,
}

impl ExpressionParser {
    fn new(expr: String) -> Self {
        ExpressionParser { expr, value: Value::Null }
    }

    fn is_expression(&self) -> bool {
        if self.expr.contains('>') || self.expr.contains('<') || self.expr.contains('=') {
            self.compare(self.value.clone())
        } else if self.expr.contains('*') {
            self.glob(self.value.clone())
        } else {
            false
        }
    }

    fn compare(&self, val: Value) -> bool {
        false
    }

    fn glob(&self, val: Value) -> bool {
        false
    }

    fn eval(&mut self, val: Value) -> bool {
        // This is a placeholder for the actual evaluation logic.
        // In a real implementation, you would parse the expression and evaluate it against the value.
        // For now, we just return true if the expression is valid.
        self.value = val;
        self.is_expression()
    }
}

/// Parse the expression and return a boolean value.
/// Example:
///
/// expr(">2GB", 4294967296)
pub fn expr(expr: &str, val: Value) -> bool {
    let mut parser = ExpressionParser::new(expr.to_string());
    if parser.is_expression() { false } else { parser.eval(val) }
}
