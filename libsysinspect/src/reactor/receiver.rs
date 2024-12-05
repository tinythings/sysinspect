/*
The role of Receiver is to accept results from the actions's modules
and collect them for a further processing (event emission, formatting, reports etc).
 */

use crate::intp::actproc::response::ActionResponse;
use indexmap::IndexMap;

#[derive(Default, Debug)]
pub struct Receiver {
    /// Storage of action results: string Id to response object.
    actions: IndexMap<String, ActionResponse>,
}

impl Receiver {
    /// Add an action.
    /// Requires:
    ///   `eid` - Entity Id
    ///   `response` - ActionResponse object
    pub fn register(&mut self, eid: String, response: ActionResponse) {
        log::debug!("Registered action response: {:#?}", response);
        self.actions.insert(eid, response);

        // XXX: And process here as well!
    }

    /// Get an action response by Entity Id
    pub fn get_by_eid(&self, eid: String) -> Option<ActionResponse> {
        self.actions.get(&eid).cloned()
    }

    /// Get all action responses in the order they were added.
    /// NOTE: the order may differ, if async is used.
    pub fn get_all(&self) -> Vec<ActionResponse> {
        let mut out: Vec<ActionResponse> = Vec::default();
        for ar in self.actions.values() {
            out.push(ar.to_owned());
        }
        out
    }
}
