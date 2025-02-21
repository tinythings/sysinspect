use crate::intp::constraints::{ConstraintKind, ExprRes};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// This struct is a future carrier of tracability.
/// Currently only a single string log message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintFailure {
    pub id: String,
    pub kind: ConstraintKind,
    pub msg: String,
    pub title: String,
}

impl ConstraintFailure {
    pub fn new(id: String, title: String, msg: String, kind: ConstraintKind) -> Self {
        ConstraintFailure { id, title, msg, kind }
    }
}

/// This struct us a future carrier of tracability.
/// It describes a constraint that successfully passed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintPass {
    pub id: String,
}

impl ConstraintPass {
    pub fn new(id: String) -> Self {
        ConstraintPass { id }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConstraintResponse {
    descr: String,
    failures: Vec<ConstraintFailure>,
    passes: Vec<ConstraintPass>,
    expr: Vec<ExprRes>,
}

impl ConstraintResponse {
    pub fn new(descr: String) -> Self {
        ConstraintResponse { descr, passes: vec![], failures: vec![], expr: vec![] }
    }

    pub fn add_failure(&mut self, fl: ConstraintFailure) {
        self.failures.push(fl);
    }

    pub fn add_pass(&mut self, c: ConstraintPass) {
        self.passes.push(c);
    }

    /// Returns `true` if the response has failures
    pub fn has_errors(&self) -> bool {
        !self.failures.is_empty()
    }

    /// Get a description of the constraint
    pub fn descr(&self) -> &str {
        &self.descr
    }

    /// Get list of failures
    pub fn failures(&self) -> &[ConstraintFailure] {
        &self.failures
    }

    /// Get a list of passes
    pub fn passes(&self) -> &[ConstraintPass] {
        &self.passes
    }

    /// Set list of evaluated expressions
    pub(crate) fn set_eval_results(&mut self, expr: Vec<ExprRes>) {
        self.expr.extend(expr);
    }

    /// Get list of evaluated expressions
    pub fn expressions(&self) -> Vec<ExprRes> {
        self.expr.to_owned()
    }

    /// Returns True if expressions do not contain any evaluated facts,
    /// but only meant to be rerouted to a specific event of a configuration
    /// management as its action module applied a specific state to the system.
    pub fn is_info(&self) -> bool {
        let mut evals = 0;
        for xpr in &self.expr {
            if !xpr.is_info() {
                evals += 1;
            }
        }

        evals == 0
    }

    /// Returns True if there is an info expression among other expressions.
    pub fn has_info(&self) -> bool {
        let mut nfo = 0;
        for xpr in &self.expr {
            if xpr.is_info() {
                nfo += 1;
            }
        }

        nfo > 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActionModResponse {
    // Return code
    retcode: i32,

    // Warnings collection
    warning: Option<Vec<String>>,

    // General message
    message: String,

    // Arbitrary payload data
    data: Option<serde_json::Value>,
}

impl ActionModResponse {
    /// Get a return code
    pub fn retcode(&self) -> i32 {
        self.retcode
    }

    /// Return collected warnings
    pub fn warnings(&self) -> Vec<String> {
        if let Some(w) = &self.warning {
            return w.to_owned();
        }
        vec![]
    }

    /// Get a general return message
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn data(&self) -> Option<Value> {
        self.data.to_owned()
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
/// This is identical to modlib::response::ModResponse but
/// can accept partially serialised data. Module does *not*
/// sends empty properties over the protocol to save the bandwidth.
pub struct ActionResponse {
    // Entity Id
    eid: String,

    // Action Id
    aid: String,

    // State Id
    sid: String,

    // Module response
    pub response: ActionModResponse,
    pub constraints: ConstraintResponse,
}

impl ActionResponse {
    pub(crate) fn new(
        eid: String, aid: String, sid: String, response: ActionModResponse, constraints: ConstraintResponse,
    ) -> Self {
        Self { eid, aid, sid, response, constraints }
    }

    /// Return an Entity Id to which this action was bound to
    pub fn eid(&self) -> &str {
        &self.eid
    }

    /// Return action Id
    pub fn aid(&self) -> &str {
        &self.aid
    }

    /// Return state Id of the action
    pub fn sid(&self) -> &str {
        if self.sid.is_empty() {
            "$"
        } else {
            &self.sid
        }
    }

    /// Match Eid.
    /// Event Id parts can be also substituted to `$` (any).
    ///
    /// Error codes:
    ///   - `$`       - any
    ///   - `0..255`  - specific code
    ///   - `E`       - error only (non-0)
    ///
    pub fn match_eid(&self, evt_id: &str) -> bool {
        // If explicitly specified and already matching
        for expr in self.constraints.expressions() {
            if let Some(ovr_evt_id) = expr.get_event_id() {
                if evt_id.eq(&ovr_evt_id) {
                    return true;
                }
            }
        }

        let p_eid = evt_id.split('/').map(|s| s.trim()).collect::<Vec<&str>>();
        p_eid.len() == 4
            && (self.aid().eq(p_eid[0]) || p_eid[0] == "$")
            && (self.eid().eq(p_eid[1]) || p_eid[1] == "$")
            && (self.sid().eq(p_eid[2]) || p_eid[2] == "$")
            && ((p_eid[3] == "$")
                || (p_eid[3].eq("E") && self.response.retcode() > 0)
                || p_eid[3].eq(&self.response.retcode().to_string()))
    }
}
