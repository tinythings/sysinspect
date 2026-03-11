use crate::{MeNotifyError, MeNotifyEventBuilder};
use mlua::{Lua, LuaSerdeExt, Scope, Table, Value as LuaValue, Variadic};
use std::{
    thread,
    time::{Duration, SystemTime},
};

/// Active host API attached to one MeNotify Lua context invocation.
#[derive(Clone, Copy)]
pub struct MeNotifyHost<'a> {
    builder: &'a MeNotifyEventBuilder,
    emit: &'a (dyn Fn(serde_json::Value) + Send + Sync),
    module: &'a str,
    sid: &'a str,
}

impl<'a> MeNotifyHost<'a> {
    /// Creates a new active host API wrapper.
    ///
    /// # Arguments
    ///
    /// * `sid` - Sensor id from the DSL.
    /// * `module` - Resolved Lua module name.
    /// * `emit` - Sensor event sink.
    /// * `builder` - Event envelope builder.
    ///
    /// # Returns
    ///
    /// Returns a new `MeNotifyHost`.
    pub fn new(sid: &'a str, module: &'a str, emit: &'a (dyn Fn(serde_json::Value) + Send + Sync), builder: &'a MeNotifyEventBuilder) -> Self {
        Self { builder, emit, module, sid }
    }

    /// Attaches active host functions to a Lua context table.
    ///
    /// # Arguments
    ///
    /// * `lua` - Lua VM that owns the context table.
    /// * `scope` - Scoped Lua lifetime used for callbacks.
    /// * `ctx` - Lua context table to extend.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all host functions are attached.
    pub fn attach<'lua>(self, lua: &'lua Lua, scope: &'lua Scope<'lua, '_>, ctx: &Table) -> Result<(), MeNotifyError>
    where
        'a: 'lua,
    {
        ctx.set("emit", self.emit_fn(lua, scope)?)?;
        ctx.set("sleep", self.sleep_fn(lua, scope)?)?;
        ctx.set("now", self.now_fn(lua, scope)?)?;
        ctx.set("timestamp", self.timestamp_fn(lua, scope)?)?;
        lua.globals().set("log", self.log_table(lua, scope)?)?;
        Ok(())
    }

    fn emit_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |lua, (data, meta): (LuaValue, Option<LuaValue>)| {
            (self.emit)(
                self.builder
                    .build(lua.from_value::<serde_json::Value>(data)?, meta.map(|v| lua.from_value::<serde_json::Value>(v)).transpose()?)
                    .map_err(|err| mlua::Error::runtime(err.to_string()))?,
            );
            Ok(())
        })?)
    }

    fn sleep_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |_, seconds: f64| {
            if !seconds.is_finite() || seconds.is_sign_negative() {
                return Err(mlua::Error::runtime("ctx.sleep(seconds) expects a non-negative finite number"));
            }
            thread::sleep(Duration::from_secs_f64(seconds));
            Ok(())
        })?)
    }

    fn now_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |_, ()| {
            Ok(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_err(|err| mlua::Error::runtime(err.to_string()))?.as_secs_f64())
        })?)
    }

    fn timestamp_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |_, ()| Ok(humantime::format_rfc3339_seconds(SystemTime::now()).to_string()))?)
    }

    fn log_table<'lua>(self, lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<Table, MeNotifyError>
    where
        'a: 'lua,
    {
        let logtbl = lua.create_table()?;
        logtbl.set("error", self.log_fn(lua, scope, "error")?)?;
        logtbl.set("warn", self.log_fn(lua, scope, "warn")?)?;
        logtbl.set("info", self.log_fn(lua, scope, "info")?)?;
        logtbl.set("debug", self.log_fn(lua, scope, "debug")?)?;
        Ok(logtbl)
    }

    fn log_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>, level: &'static str) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |_, vals: Variadic<LuaValue>| {
            let msg = format!("[menotify] '{}' [{}] {}", self.sid, self.module, Self::join_vals(vals));
            match level {
                "error" => log::error!("{msg}"),
                "warn" => log::warn!("{msg}"),
                "info" => log::info!("{msg}"),
                _ => log::debug!("{msg}"),
            }
            Ok(())
        })?)
    }

    fn join_vals(vals: Variadic<LuaValue>) -> String {
        vals.into_iter().map(Self::value_to_string).collect::<Vec<_>>().join(" ")
    }

    fn value_to_string(v: LuaValue) -> String {
        match v {
            LuaValue::Nil => "nil".to_string(),
            LuaValue::Boolean(v) => v.to_string(),
            LuaValue::Integer(v) => v.to_string(),
            LuaValue::Number(v) => v.to_string(),
            LuaValue::String(v) => v.to_string_lossy().to_string(),
            other => format!("<lua:{}>", other.type_name()),
        }
    }
}
