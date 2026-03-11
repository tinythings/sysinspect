use crate::{MeNotifyError, MeNotifyHost};
use mlua::{Lua, Scope, Table, Value as LuaValue};
use serde_yaml::{Mapping, Value as YamlValue};
use std::time::Duration;

/// Passive runtime context passed to one MeNotify Lua entrypoint call.
#[derive(Debug, Clone)]
pub struct MeNotifyContext {
    sid: String,
    listener: String,
    module: String,
    opts: Vec<String>,
    args: YamlValue,
    interval: Option<Duration>,
}

impl MeNotifyContext {
    /// Creates a new passive MeNotify context.
    ///
    /// # Arguments
    ///
    /// * `sid` - Sensor id from the DSL.
    /// * `listener` - Full listener string, for example `menotify.demo`.
    /// * `module` - Resolved module name.
    /// * `opts` - Listener options from the DSL.
    /// * `args` - Listener arguments from the DSL.
    /// * `interval` - Effective interval, if configured.
    ///
    /// # Returns
    ///
    /// Returns a new `MeNotifyContext`.
    pub fn new(sid: &str, listener: &str, module: &str, opts: &[String], args: &YamlValue, interval: Option<Duration>) -> Self {
        Self { sid: sid.to_string(), listener: listener.to_string(), module: module.to_string(), opts: opts.to_vec(), args: args.clone(), interval }
    }

    /// Returns the sensor id.
    ///
    /// # Returns
    ///
    /// Returns the DSL sensor id.
    pub fn sid(&self) -> &str {
        &self.sid
    }

    /// Returns the listener string.
    ///
    /// # Returns
    ///
    /// Returns the full listener string.
    pub fn listener(&self) -> &str {
        &self.listener
    }

    /// Returns the resolved module name.
    ///
    /// # Returns
    ///
    /// Returns the Lua module name.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// Returns the configured listener options.
    ///
    /// # Returns
    ///
    /// Returns the configured options as a slice.
    pub fn opts(&self) -> &[String] {
        &self.opts
    }

    /// Returns the configured listener arguments.
    ///
    /// # Returns
    ///
    /// Returns the raw YAML arguments.
    pub fn args(&self) -> &YamlValue {
        &self.args
    }

    /// Returns the configured interval.
    ///
    /// # Returns
    ///
    /// Returns the configured interval, if any.
    pub fn interval(&self) -> Option<Duration> {
        self.interval
    }

    /// Builds a Lua table for the passive v1 context.
    ///
    /// # Arguments
    ///
    /// * `lua` - Lua VM that will own the produced table.
    ///
    /// # Returns
    ///
    /// Returns a Lua table containing passive context fields only.
    pub fn to_lua(&self, lua: &Lua) -> Result<Table, MeNotifyError> {
        let ctx = lua.create_table()?;
        self.fill(ctx.clone(), lua)?;
        Ok(ctx)
    }

    /// Builds a Lua table for the passive v1 context and attaches scoped `emit`.
    ///
    /// # Arguments
    ///
    /// * `lua` - Lua VM that will own the produced table.
    /// * `scope` - Scoped Lua lifetime used for non-static callbacks.
    /// * `emit` - Sensor event sink.
    /// * `builder` - Event envelope builder.
    ///
    /// # Returns
    ///
    /// Returns a Lua table containing passive context fields and `emit`.
    pub fn to_lua_scoped<'lua>(
        &'lua self, lua: &'lua Lua, scope: &'lua Scope<'lua, '_>, emit: &'lua (dyn Fn(serde_json::Value) + Send + Sync),
        builder: &'lua crate::MeNotifyEventBuilder,
    ) -> Result<Table, MeNotifyError> {
        let ctx = lua.create_table()?;
        self.fill(ctx.clone(), lua)?;
        let host = MeNotifyHost::new(self.sid(), self.module(), emit, builder);
        host.attach(lua, scope, &ctx)?;
        Ok(ctx)
    }

    fn fill(&self, ctx: Table, lua: &Lua) -> Result<(), MeNotifyError> {
        ctx.set("id", self.sid())?;
        ctx.set("listener", self.listener())?;
        ctx.set("module", self.module())?;
        ctx.set("opts", self.opts_to_lua(lua)?)?;
        ctx.set("args", Self::yaml_to_lua(lua, self.args())?)?;
        if let Some(interval) = self.interval() {
            ctx.set("interval", interval.as_secs_f64())?;
        }
        Ok(())
    }

    fn opts_to_lua(&self, lua: &Lua) -> Result<Table, MeNotifyError> {
        let out = lua.create_table()?;
        for (idx, opt) in self.opts().iter().enumerate() {
            out.set(idx + 1, opt.as_str())?;
        }
        Ok(out)
    }

    fn mapping_key(v: &YamlValue) -> String {
        match v {
            YamlValue::String(s) => s.clone(),
            YamlValue::Number(n) => n.to_string(),
            YamlValue::Bool(b) => b.to_string(),
            YamlValue::Null => "null".to_string(),
            other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
        }
    }

    fn mapping_to_lua(lua: &Lua, mapping: &Mapping) -> Result<Table, MeNotifyError> {
        let out = lua.create_table()?;
        for (k, v) in mapping {
            out.set(Self::mapping_key(k), Self::yaml_to_lua(lua, v)?)?;
        }
        Ok(out)
    }

    fn sequence_to_lua(lua: &Lua, seq: &[YamlValue]) -> Result<Table, MeNotifyError> {
        let out = lua.create_table()?;
        for (idx, v) in seq.iter().enumerate() {
            out.set(idx + 1, Self::yaml_to_lua(lua, v)?)?;
        }
        Ok(out)
    }

    fn yaml_to_lua(lua: &Lua, v: &YamlValue) -> Result<LuaValue, MeNotifyError> {
        Ok(match v {
            YamlValue::Null => LuaValue::Nil,
            YamlValue::Bool(v) => LuaValue::Boolean(*v),
            YamlValue::Number(v) if v.is_i64() => LuaValue::Integer(v.as_i64().unwrap_or_default()),
            YamlValue::Number(v) => LuaValue::Number(v.as_f64().unwrap_or_default()),
            YamlValue::String(v) => LuaValue::String(lua.create_string(v)?),
            YamlValue::Sequence(v) => LuaValue::Table(Self::sequence_to_lua(lua, v)?),
            YamlValue::Mapping(v) => LuaValue::Table(Self::mapping_to_lua(lua, v)?),
            YamlValue::Tagged(v) => Self::yaml_to_lua(lua, &v.value)?,
        })
    }
}
