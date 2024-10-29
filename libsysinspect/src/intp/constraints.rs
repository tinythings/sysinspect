use crate::{
    util::dataconv::{as_bool_opt, as_int_opt, as_str_opt},
    SysinspectError,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;

pub struct ExprRes {
    result: bool,
    traces: Vec<String>,
}

impl ExprRes {
    /// Create a new instance of ExpressionResult
    pub fn new(result: Option<bool>, trace: Option<String>) -> Self {
        ExprRes {
            result: result.unwrap_or(false),
            traces: if trace.is_some() { vec![trace.unwrap_or_default()] } else { vec![] },
        }
    }

    /// Add trace to the stack
    pub fn add_trace(&mut self, msg: String) {
        _ = &self.traces.push(msg);
    }

    /// Get existing traces
    pub fn traces(&self) -> &[String] {
        &self.traces
    }

    /// Get the outcome
    pub fn is_positive(&self) -> bool {
        self.result
    }
}

#[derive(Eq, PartialEq, Hash)]
enum OpType {
    // Operators
    Equals,
    Less,
    More,
    Matches,
    Contains,
    Starts,
    Ends,

    // No expression defined
    Undef,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Expression {
    /// Namespace of a fact to the output structure
    fact: String,

    /// Operations
    equals: Option<Value>,
    less: Option<Value>,
    more: Option<Value>,
    matches: Option<Value>,
    contains: Option<Value>,
    starts: Option<Value>,
    ends: Option<Value>,
}

impl Expression {
    /// Validate the expression: only one expression can be defined.
    pub fn is_valid(&self) -> bool {
        [&self.equals, &self.less, &self.more, &self.matches, &self.contains, &self.starts, &self.ends]
            .iter()
            .filter(|&op| op.is_some())
            .count()
            == 1
    }

    /// Get active operator
    fn op(&self) -> Option<(OpType, Value)> {
        for (k, v) in HashMap::from([
            (OpType::Equals, &self.equals),
            (OpType::Less, &self.less),
            (OpType::More, &self.more),
            (OpType::Matches, &self.matches),
            (OpType::Contains, &self.contains),
            (OpType::Starts, &self.starts),
            (OpType::Ends, &self.ends),
        ]) {
            if let Some(v) = v {
                return Some((k, v.to_owned()));
            }
        }

        None
    }

    /// Get active operator w/o type
    pub fn get_op(&self) -> Option<Value> {
        let op = self.op();
        if let Some((_, op)) = op {
            return Some(op);
        }

        None
    }

    /// Set to active operator. If no operator is defined yet (all `None`), error is returned
    pub fn set_active_op(&mut self, eq: Value) -> Result<(), SysinspectError> {
        for op_ref in [
            &mut self.equals,
            &mut self.less,
            &mut self.more,
            &mut self.matches,
            &mut self.contains,
            &mut self.starts,
            &mut self.ends,
        ] {
            if op_ref.is_some() {
                *op_ref = Some(eq.clone());
                return Ok(());
            }
        }

        // This must never happen
        Err(SysinspectError::ModelDSLError("Constraint has no active operator!".to_string()))
    }

    /// Get fact namespace
    pub fn get_fact_namespace(&self) -> String {
        self.fact.to_owned()
    }

    /// Evaluate operator with the given fact data
    /// `fact` is incoming data from the plugin output.
    pub fn eval(&self, fact: Option<serde_json::Value>) -> ExprRes {
        // XXX: Eval() should also get namespaces and constraint name,
        //      so then tracing can be built nicely.
        if fact.is_none() {
            return ExprRes::new(Some(false), Some("No facts to evaluate".to_string()));
        }
        let fact = fact.unwrap();

        // Module data is a "fact", compared to the "claim" from the model.
        let (op, claim) = self.op().unwrap_or_else(|| (OpType::Undef, Value::default()));
        if op == OpType::Undef {
            return ExprRes::new(Some(false), Some("Undefined expression".to_string()));
        }

        let v_claim = Some(&claim).cloned();

        match fact {
            serde_json::Value::Null => ExprRes::new(Some(fact.is_null()), Some("No facts to evaluate".to_string())),
            serde_json::Value::Bool(fact) => {
                let claim = as_bool_opt(v_claim);
                if claim.is_none() {
                    return ExprRes::new(Some(false), Some("Could not obtain claim value as boolean".to_string()));
                }
                let claim = claim.unwrap();

                match op {
                    OpType::Equals => ExprRes::new(Some(fact == claim), None),
                    OpType::Less | OpType::More => ExprRes::new(Some(fact != claim), None),
                    _ => ExprRes::new(Some(false), Some("Unknown expression operator".to_string())),
                }
            }
            serde_json::Value::Number(_) => {
                let fact = as_int_opt(Some(fact.to_owned()));
                if fact.is_none() {
                    return ExprRes::new(Some(false), Some("Could not obtain fact value as a number".to_string()));
                }
                let fact = fact.unwrap();
                let claim = as_int_opt(v_claim.clone());
                if claim.is_none() {
                    return ExprRes::new(Some(false), Some("Could not obtain claim value as a number".to_string()));
                }
                let claim = claim.unwrap();

                match op {
                    OpType::Equals => ExprRes::new(Some(fact == claim), Some(format!("{} should be equal to {}", fact, claim))),
                    OpType::Less => ExprRes::new(Some(fact < claim), Some(format!("{} should be less than {}", fact, claim))),
                    OpType::More => ExprRes::new(Some(fact > claim), Some(format!("{} should be more than {}", fact, claim))),
                    _ => ExprRes::new(None, Some("Unknown expression operator".to_string())),
                }
            }
            serde_json::Value::String(fact) => {
                let claim = as_str_opt(v_claim);
                if claim.is_none() {
                    return ExprRes::new(Some(false), Some("Could not obtain claim value as a string".to_string()));
                }
                let claim = claim.unwrap_or_default();

                match op {
                    OpType::Equals => ExprRes::new(Some(claim.eq(&fact)), Some(format!("{} should be equal to {}", claim, fact))),
                    OpType::Less | OpType::More => {
                        ExprRes::new(Some(claim.ne(&fact)), Some(format!("{} should not be equal to {}", claim, fact)))
                    }
                    OpType::Matches => {
                        if let Ok(r) = Regex::new(&claim) {
                            ExprRes::new(Some(r.is_match(&fact)), Some(format!("{} should match {}", fact, claim)))
                        } else {
                            ExprRes::new(None, Some("Bad regexp syntax".to_string()))
                        }
                    }
                    OpType::Contains => {
                        ExprRes::new(Some(claim.contains(&fact)), Some(format!("{} should contain {}", fact, claim)))
                    }
                    OpType::Starts => {
                        ExprRes::new(Some(claim.starts_with(&fact)), Some(format!("{} should start with {}", fact, claim)))
                    }
                    OpType::Ends => {
                        ExprRes::new(Some(claim.ends_with(&fact)), Some(format!("{} should ends with {}", fact, claim)))
                    }
                    _ => ExprRes::new(None, None),
                }
            }
            _ => ExprRes::new(None, None),
        }
    }

    /// Get value from a JSON structure by the namespace
    pub fn get_by_namespace(data: Option<serde_json::Value>, namespace: &str) -> Option<serde_json::Value> {
        if let Some(ref data) = data {
            let ns: Vec<&str> = namespace.split('.').collect();

            if let Some(v) = Self::get_ns_val(data, &ns) {
                return Some(v);
            }
        }

        None
    }

    /// Recursively walk a JSON value to extract its content by a parsed namespace
    fn get_ns_val(data: &serde_json::Value, ns: &[&str]) -> Option<serde_json::Value> {
        for n in ns {
            match data {
                serde_json::Value::Array(data) => {
                    for v in data {
                        if let Some(v) = Self::get_ns_val(v, ns) {
                            return Some(v.to_owned());
                        } else {
                            Self::get_ns_val(v, ns);
                        }
                    }
                }
                serde_json::Value::Object(data) => {
                    if let Some(v) = data.get(&n.to_string()) {
                        return Self::get_ns_val(v, ns);
                    }
                }
                _ => return Some(data.to_owned()),
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub enum ConstraintKind {
    All,
    Any,
    None,
}
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Constraint {
    id: Option<String>,
    descr: Option<String>,
    entities: Option<Vec<String>>,

    // All of the expressions should match for positive outcome
    all: Option<HashMap<String, Vec<Expression>>>,

    // Any of the defined expressions should match for positive outcome
    any: Option<HashMap<String, Vec<Expression>>>,

    // None of the defined expressions must match for positive outcome
    none: Option<HashMap<String, Vec<Expression>>>,
}

impl Constraint {
    pub fn new(id: &Value, constraint: &Value) -> Result<Self, SysinspectError> {
        let mut instance = Constraint::default();
        let c_id: String;

        if let Some(id) = id.as_str() {
            c_id = id.to_string();
        } else {
            return Err(SysinspectError::ModelDSLError("no ID assigned".to_string()));
        }

        if let Ok(mut c) = serde_yaml::from_value::<Constraint>(constraint.to_owned()) {
            c.id = Some(c_id);
            instance = c;
        }

        if instance.entities.is_none() {
            return Err(SysinspectError::ModelDSLError(format!(
                "\"{}\" has no entities defined, implying global scope.",
                &instance.id()
            )));
        }

        if instance.all.is_none() && instance.any.is_none() && instance.none.is_none() {
            return Err(SysinspectError::ModelDSLError(format!("\"{}\" has no any expressions defined", &instance.id())));
        }

        Ok(instance)
    }

    /// Get `id` of the Constraint
    pub fn id(&self) -> String {
        self.id.to_owned().unwrap_or("".to_string())
    }

    /// Get `description` of the Constraint.
    /// Field is **optional**.
    pub fn descr(&self) -> String {
        self.descr.to_owned().unwrap_or("".to_string())
    }

    /// Check if an action has any entity that would bind to this constraint
    ///
    /// Rules:
    ///   - `$` and and Entity Id = "all, except that entity"
    ///   - `$` alone means "all"
    ///   - Any entity means "only these entities"
    pub fn binds_to_any(&self, a_eids: &Vec<String>) -> bool {
        for eid in a_eids {
            if self.binds_to(eid) {
                return true;
            }
        }

        false
    }

    /// Return True if a constraint binds to a given entity
    fn binds_to(&self, entity: &str) -> bool {
        let entities = self.entities.clone().unwrap();
        let has_glob = entities.contains(&"$".to_string());
        let has_entity = entities.contains(&entity.to_string());

        (entities.len() == 1 && has_glob) || (entities.len() > 1 && has_glob && !has_entity) || (!has_glob && has_entity)
    }

    fn get_expr(&self, state: String, expr: &Option<HashMap<String, Vec<Expression>>>) -> Vec<Expression> {
        let mut out: Vec<Expression> = Vec::default();
        if let Some(expr) = expr {
            if let Some(exprset) = expr.get(&state) {
                out.extend(exprset.iter().cloned());
            }
        }

        out
    }

    /// Get all expressions for "any"
    pub fn any(&self, state: String) -> Vec<Expression> {
        self.get_expr(state, &self.any)
    }

    /// Get all expressions for "all"
    pub fn all(&self, state: String) -> Vec<Expression> {
        self.get_expr(state, &self.all)
    }

    /// Get all expressions for "none"
    pub fn none(&self, state: String) -> Vec<Expression> {
        self.get_expr(state, &self.none)
    }

    /// Update resolved expressions for the specific state.
    ///
    /// This method is used after claims/functions/namespaces are resolved and replaced with the
    /// real values, so the expression is ready for the evaluation.
    pub fn set_expr_for(&mut self, state: String, expr: Vec<Expression>, kind: ConstraintKind) {
        match kind {
            ConstraintKind::All => self.all.get_or_insert_with(HashMap::new).insert(state, expr),
            ConstraintKind::Any => self.any.get_or_insert_with(HashMap::new).insert(state, expr),
            ConstraintKind::None => self.none.get_or_insert_with(HashMap::new).insert(state, expr),
        };
    }
}
