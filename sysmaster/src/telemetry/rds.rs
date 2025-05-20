use indexmap::IndexMap;
use serde_json::Value;

pub struct FunctionReducer {
    fmap: IndexMap<String, String>,
    data: IndexMap<String, Value>,
}

impl FunctionReducer {
    pub fn new(map: IndexMap<String, String>) -> Self {
        FunctionReducer { fmap: map, data: IndexMap::new() }
    }

    pub(crate) fn set_data(mut self, data: IndexMap<String, Value>) -> Self {
        self.data = data;
        self
    }

    /// Run the function over values.
    /// Consume self, apply each mapping in-place to `self.data`, and return it.
    pub(crate) fn reduce(&self) -> IndexMap<String, Value> {
        let mut out = IndexMap::new();
        log::info!("Data for reduction: {:#?}", self.data);
        out
    }
}
