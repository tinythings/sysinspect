/// Active box selector
#[derive(Debug, PartialEq, Eq)]
pub enum ActiveBox {
    Cycles,
    Minions,
    Events,
    Info,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AlertResult {
    Default,
    Purge,
    Quit,
}

pub trait DbListItem {
    fn title(&self) -> String;
    fn id(&self) -> u32;
}

/// Cycle
/// -----
///
/// Cycle is the pointer in the list to the record in the database
#[derive(Debug, Clone)]
pub struct CycleListItem {
    id: u32,
    title: String,
}

impl CycleListItem {
    pub fn new(title: &str, id: u32) -> Self {
        CycleListItem { id, title: title.to_string() }
    }
}

impl DbListItem for CycleListItem {
    fn title(&self) -> String {
        format!("{} {}", self.title, self.id())
    }

    fn id(&self) -> u32 {
        self.id
    }
}

/// Event
/// -----
#[derive(Debug, Clone)]
pub struct EventListItem {
    id: u32,
    title: String,
}

impl EventListItem {
    pub fn new(title: &str, id: u32) -> Self {
        EventListItem { id, title: title.to_string() }
    }
}

impl DbListItem for EventListItem {
    fn title(&self) -> String {
        format!("{}: {}", self.title, self.id())
    }

    fn id(&self) -> u32 {
        self.id
    }
}

/// Minion
/// ------
#[derive(Debug, Clone)]
pub struct MinionListItem {
    id: u32,
    title: String,
}

impl MinionListItem {
    pub fn new(title: &str, id: u32) -> Self {
        MinionListItem { id, title: title.to_string() }
    }
}

impl DbListItem for MinionListItem {
    fn title(&self) -> String {
        format!("{}: {}", self.title, self.id())
    }

    fn id(&self) -> u32 {
        self.id
    }
}
