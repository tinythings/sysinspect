use crate::{
    MeNotifyError, MeNotifyEventBuilder,
    helpers::{MeNotifyHttp, MeNotifyHttpRequestSpec, MeNotifyPackageKit},
};
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
        lua.globals().set("http", self.http_table(lua, scope)?)?;
        lua.globals().set("log", self.log_table(lua, scope)?)?;
        lua.globals().set("packagekit", self.packagekit_table(lua, scope)?)?;
        Ok(())
    }

    fn http_table<'lua>(self, lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<Table, MeNotifyError>
    where
        'a: 'lua,
    {
        let httptbl = lua.create_table()?;
        httptbl.set("get", self.http_get_fn(lua, scope)?)?;
        httptbl.set("request", self.http_request_fn(lua, scope)?)?;
        Ok(httptbl)
    }

    fn http_get_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |lua, (url, opts): (String, Option<LuaValue>)| {
            Self::request(
                &MeNotifyHttpRequestSpec::from_get(url, opts.map(|v| lua.from_value::<serde_json::Value>(v)).transpose()?)
                    .map_err(|err| mlua::Error::runtime(err.to_string()))?,
            )
            .and_then(|rsp| lua.to_value(&rsp).map_err(MeNotifyError::from))
            .map_err(|err| mlua::Error::runtime(err.to_string()))
        })?)
    }

    fn http_request_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |lua, spec: LuaValue| {
            Self::request(
                &serde_json::from_value::<MeNotifyHttpRequestSpec>(lua.from_value::<serde_json::Value>(spec)?)
                    .map_err(|err| mlua::Error::runtime(format!("http.request({{...}}) invalid request spec: {err}")))?,
            )
            .and_then(|rsp| lua.to_value(&rsp).map_err(MeNotifyError::from))
            .map_err(|err| mlua::Error::runtime(err.to_string()))
        })?)
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

    fn packagekit_table<'lua>(self, lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<Table, MeNotifyError>
    where
        'a: 'lua,
    {
        let pktbl = lua.create_table()?;
        pktbl.set("available", self.packagekit_available_fn(lua, scope)?)?;
        pktbl.set("packages", self.packagekit_packages_fn(lua, scope)?)?;
        pktbl.set("status", self.packagekit_status_fn(lua, scope)?)?;
        pktbl.set("history", self.packagekit_history_fn(lua, scope)?)?;
        Ok(pktbl)
    }

    fn packagekit_available_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |_, ()| Ok(MeNotifyPackageKit::available()))?)
    }

    fn packagekit_status_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |lua, ()| {
            MeNotifyPackageKit::status()
                .and_then(|status| lua.to_value(&status).map_err(MeNotifyError::from))
                .map_err(|err| mlua::Error::runtime(err.to_string()))
        })?)
    }

    fn packagekit_packages_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |lua, ()| {
            MeNotifyPackageKit::packages()
                .and_then(|packages| lua.to_value(&packages).map_err(MeNotifyError::from))
                .map_err(|err| mlua::Error::runtime(err.to_string()))
        })?)
    }

    fn packagekit_history_fn<'lua>(self, _lua: &'lua Lua, scope: &'lua Scope<'lua, '_>) -> Result<mlua::Function, MeNotifyError>
    where
        'a: 'lua,
    {
        Ok(scope.create_function(move |lua, (names, count): (Vec<String>, Option<u32>)| {
            MeNotifyPackageKit::history(names, count.unwrap_or(10))
                .and_then(|history| lua.to_value(&history).map_err(MeNotifyError::from))
                .map_err(|err| mlua::Error::runtime(err.to_string()))
        })?)
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

    #[cfg(test)]
    pub(crate) fn timeout_for_test(timeout: f64) -> Result<Duration, MeNotifyError> {
        MeNotifyHttp::timeout(&MeNotifyHttpRequestSpec { timeout, ..MeNotifyHttpRequestSpec::default() })
    }

    fn request(spec: &MeNotifyHttpRequestSpec) -> Result<crate::helpers::MeNotifyHttpResponse, MeNotifyError> {
        MeNotifyHttp::request(spec)
    }
}
