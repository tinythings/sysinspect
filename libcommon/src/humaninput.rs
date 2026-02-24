use serde::{
    Deserialize,
    de::{self, Deserializer},
};

/// Custom deserializer for byte sizes that can be specified as either a number (in bytes) or a human-readable string (e.g. "10MB").
/// This is used for deserializing the `max_item_size` and `max_overall_size` fields in the configuration, allowing users to specify
/// sizes in a more convenient way. The deserializer accepts either a numeric value (interpreted as bytes) or a string with an optional
/// unit suffix (e.g. "KB", "MB", "GB"). If the input is a string, it will be parsed using the `parse_size` function, which supports
/// common size suffixes and converts them to bytes.
///
/// Example usage in a struct:
/// ```
/// #[derive(Deserialize)]
/// struct Config {
///     #[serde(deserialize_with = "h2bytes")]
///     max_item_size: Option<u64>,
/// }
/// ```
pub fn h2bytes<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum V {
        Str(String),
        Num(u64),
    }

    let v = Option::<V>::deserialize(deserializer)?;
    match v {
        None => Ok(None),
        Some(V::Num(n)) => Ok(Some(n)),
        Some(V::Str(s)) => parse_size::parse_size(&s).map(Some).map_err(|e| de::Error::custom(format!("bad size '{s}': {e}"))),
    }
}
