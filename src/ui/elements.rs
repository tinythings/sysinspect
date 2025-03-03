/// Active box selector
#[derive(Debug, PartialEq, Eq)]
pub enum ActiveBox {
    Cycles,
    Minions,
    Events,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AlertResult {
    Default,
    Purge,
    Quit,
}

/// Cycle is the pointer in the list to the record in the database
#[derive(Debug, Clone)]
pub struct Cycle {
    pub id: u32,
}

impl Cycle {
    pub fn get_title(&self) -> String {
        format!("cycle {}", self.id)
    }
}
