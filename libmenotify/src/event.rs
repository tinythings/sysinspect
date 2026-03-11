use crate::MeNotifyError;
use serde::Deserialize;
use serde_json::{Value, json};

/// Optional metadata accepted by `ctx.emit(data, meta?)`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MeNotifyEventMeta {
    action: Option<String>,
    key: Option<String>,
}

impl MeNotifyEventMeta {
    /// Returns the resolved action name.
    ///
    /// # Returns
    ///
    /// Returns the action name, or `"emitted"` when omitted.
    pub fn action(&self) -> &str {
        self.action.as_deref().unwrap_or("emitted")
    }

    /// Returns the resolved key value.
    ///
    /// # Returns
    ///
    /// Returns the key, or `"-"` when omitted.
    pub fn key(&self) -> &str {
        self.key.as_deref().unwrap_or("-")
    }
}

/// Builds standard Sysinspect sensor envelopes for MeNotify emissions.
#[derive(Debug, Clone)]
pub struct MeNotifyEventBuilder {
    listener: String,
    listener_id: String,
    sid: String,
}

impl MeNotifyEventBuilder {
    /// Creates a new event builder.
    ///
    /// # Arguments
    ///
    /// * `sid` - Sensor id from the DSL.
    /// * `listener` - Full listener string, for example `menotify.demo`.
    /// * `tag` - Optional sensor tag.
    ///
    /// # Returns
    ///
    /// Returns a new `MeNotifyEventBuilder`.
    pub fn new(sid: &str, listener: &str, tag: Option<&str>) -> Self {
        Self {
            listener: listener.to_string(),
            listener_id: format!("{listener}{}{}", if tag.is_none() { "" } else { "@" }, tag.unwrap_or("")),
            sid: sid.to_string(),
        }
    }

    /// Returns the configured sensor id.
    ///
    /// # Returns
    ///
    /// Returns the DSL sensor id.
    pub fn sid(&self) -> &str {
        &self.sid
    }

    /// Returns the configured listener string.
    ///
    /// # Returns
    ///
    /// Returns the full listener string.
    pub fn listener(&self) -> &str {
        &self.listener
    }

    /// Returns the listener id used in generated EIDs.
    ///
    /// # Returns
    ///
    /// Returns the listener string with optional `@tag`.
    pub fn listener_id(&self) -> &str {
        &self.listener_id
    }

    /// Parses optional metadata passed to `ctx.emit`.
    ///
    /// # Arguments
    ///
    /// * `meta` - Optional JSON value passed as the second emit argument.
    ///
    /// # Returns
    ///
    /// Returns normalized event metadata.
    pub fn parse_meta(meta: Option<Value>) -> Result<MeNotifyEventMeta, MeNotifyError> {
        meta.map(serde_json::from_value)
            .transpose()
            .map_err(|source| MeNotifyError::InvalidEmitMeta(source.to_string()))
            .map(|meta| meta.unwrap_or_default())
    }

    /// Builds one standard sensor event envelope.
    ///
    /// # Arguments
    ///
    /// * `data` - JSON payload emitted from Lua.
    /// * `meta` - Optional metadata emitted from Lua.
    ///
    /// # Returns
    ///
    /// Returns a standard Sysinspect sensor event envelope.
    pub fn build(&self, data: Value, meta: Option<Value>) -> Result<Value, MeNotifyError> {
        let meta = Self::parse_meta(meta)?;
        Ok(json!({
            "eid": format!("{}|{}|{}@{}|{}", self.sid(), self.listener_id(), meta.action(), meta.key(), 0),
            "sensor": self.sid(),
            "listener": self.listener(),
            "data": data,
        }))
    }
}
