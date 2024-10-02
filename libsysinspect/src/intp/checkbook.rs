use super::entities::Entity;
use std::collections::HashMap;

pub struct Checkbook {
    entities: HashMap<String, Entity>,
}

impl Checkbook {
    /// Initialise a checkbook.
    /// Entry is a list of relations needs to be examined.
    pub fn new(entry: Vec<String>) -> Self {
        Checkbook { entities: HashMap::new() }.load(entry)
    }

    fn load(self, entry: Vec<String>) -> Self {
        self
    }
}
