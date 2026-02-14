/*
The role of Receiver is to accept results from the actions's modules
and collect them for a further processing (event emission, formatting, reports etc).
 */

use crate::intp::actproc::response::ActionResponse;
use indexmap::IndexMap;

#[derive(Default, Debug)]
pub struct Receiver {
    /// Storage of action results: string Id to response object.
    actions: IndexMap<String, Vec<ActionResponse>>,
}

impl Receiver {
    /// Add an action.
    /// NOTE: the order of responses is not guaranteed, if async is used.
    ///
    /// Parameters:
    /// - `eid`: Entity Id, which is a string identifier of the entity that produced the response. It can be used to group responses by their source.
    /// - `response`: The actual response object, which contains the data produced by the action. It can be of any type that implements the `ActionResponse` trait.
    ///
    /// Returns: None. This method modifies the internal state of the `Receiver` by adding the response to the list of responses associated with the given Entity Id.
    pub fn register(&mut self, eid: String, response: ActionResponse) {
        // XXX: And process here as well!
        log::debug!("Registered action response: {response:#?}");
        self.actions.entry(eid).or_default().push(response);
    }

    /// Drain all action responses (consumes stored ones).
    pub fn drain_all(&mut self) -> Vec<ActionResponse> {
        let mut out = Vec::new();
        for (_, mut v) in self.actions.drain(..) {
            out.append(&mut v);
        }
        out
    }

    /// Get an action response by Entity Id
    pub fn get_by_eid(&self, eid: String) -> Option<Vec<ActionResponse>> {
        self.actions.get(&eid).cloned()
    }

    /// Get all action responses in the order they were added.
    /// NOTE: the order may differ, if async is used.
    pub fn get_all(&self) -> Vec<ActionResponse> {
        let mut out: Vec<ActionResponse> = Vec::default();
        for ar in self.actions.values() {
            out.extend(ar.to_owned());
        }
        out
    }

    pub fn get_last(&self) -> Option<ActionResponse> {
        self.actions.values().last().and_then(|v| v.last().cloned())
    }
}
