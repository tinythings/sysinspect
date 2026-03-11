use crate::MeNotifyError;
use mlua::{Lua, LuaSerdeExt, Table, Value as LuaValue};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// VM-local in-memory key/value state for one configured sensor instance.
#[derive(Debug, Clone, Default)]
pub struct MeNotifyState {
    data: Arc<Mutex<BTreeMap<String, Value>>>,
}

impl MeNotifyState {
    /// Creates a new empty state store.
    ///
    /// # Returns
    ///
    /// Returns a new `MeNotifyState`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if a key exists in the state store.
    ///
    /// # Arguments
    ///
    /// * `key` - State key.
    ///
    /// # Returns
    ///
    /// Returns `true` if the key exists.
    pub fn has(&self, key: &str) -> bool {
        self.data.lock().expect("state lock should not poison").contains_key(key)
    }

    /// Returns a cloned value for the given key.
    ///
    /// # Arguments
    ///
    /// * `key` - State key.
    ///
    /// # Returns
    ///
    /// Returns `Some(Value)` if the key exists, otherwise `None`.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.data.lock().expect("state lock should not poison").get(key).cloned()
    }

    /// Sets a state value.
    ///
    /// # Arguments
    ///
    /// * `key` - State key.
    /// * `value` - JSON value to store.
    ///
    /// # Returns
    ///
    /// Returns nothing.
    pub fn set(&self, key: &str, value: Value) {
        self.data.lock().expect("state lock should not poison").insert(key.to_string(), value);
    }

    /// Deletes a state key.
    ///
    /// # Arguments
    ///
    /// * `key` - State key.
    ///
    /// # Returns
    ///
    /// Returns `true` if the key existed and was removed.
    pub fn del(&self, key: &str) -> bool {
        self.data.lock().expect("state lock should not poison").remove(key).is_some()
    }

    /// Builds a Lua table exposing state helpers.
    ///
    /// # Arguments
    ///
    /// * `lua` - Lua VM that will own the produced table.
    ///
    /// # Returns
    ///
    /// Returns a Lua table containing `get`, `set`, `has`, and `del`.
    pub fn to_lua(&self, lua: &Lua) -> Result<Table, MeNotifyError> {
        let state = lua.create_table()?;
        let store = self.clone();
        state.set(
            "get",
            lua.create_function(move |lua, key: String| store.get(&key).map(|v| lua.to_value(&v)).transpose().map(|v| v.unwrap_or(LuaValue::Nil)))?,
        )?;

        let store = self.clone();
        state.set(
            "set",
            lua.create_function(move |lua, (key, value): (String, LuaValue)| {
                store.set(&key, lua.from_value::<Value>(value)?);
                Ok(())
            })?,
        )?;

        let store = self.clone();
        state.set("has", lua.create_function(move |_, key: String| Ok(store.has(&key)))?)?;

        let store = self.clone();
        state.set("del", lua.create_function(move |_, key: String| Ok(store.del(&key)))?)?;
        Ok(state)
    }
}

#[cfg(test)]
mod state_ut;
