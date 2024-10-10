use serde_json::Value;

pub trait StringFormatter {
    fn new(data: Value) -> Self
    where
        Self: Sized;

    /// Format the output
    fn format(&self) -> String;
}
