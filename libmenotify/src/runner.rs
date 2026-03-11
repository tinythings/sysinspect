use crate::{MeNotifyContext, MeNotifyEntrypoint, MeNotifyError, MeNotifyEventBuilder, MeNotifyProgram};

/// Execution wrapper for one loaded MeNotify program and one sensor context.
#[derive(Debug)]
pub struct MeNotifyRunner {
    ctx: MeNotifyContext,
    program: MeNotifyProgram,
}

impl MeNotifyRunner {
    /// Creates a new MeNotify runner.
    ///
    /// # Arguments
    ///
    /// * `program` - Loaded and validated MeNotify program.
    /// * `ctx` - Passive execution context for the configured sensor.
    ///
    /// # Returns
    ///
    /// Returns a new `MeNotifyRunner`.
    pub fn new(program: MeNotifyProgram, ctx: MeNotifyContext) -> Self {
        Self { ctx, program }
    }

    /// Returns the configured execution context.
    ///
    /// # Returns
    ///
    /// Returns the passive execution context.
    pub fn ctx(&self) -> &MeNotifyContext {
        &self.ctx
    }

    /// Returns the loaded program.
    ///
    /// # Returns
    ///
    /// Returns the loaded program.
    pub fn program(&self) -> &MeNotifyProgram {
        &self.program
    }

    /// Returns the selected entrypoint kind.
    ///
    /// # Returns
    ///
    /// Returns the validated entrypoint type.
    pub fn entrypoint(&self) -> MeNotifyEntrypoint {
        self.program().contract().entrypoint()
    }

    /// Calls one `tick(ctx)` entrypoint invocation.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the Lua function completed successfully.
    pub fn run_tick(&self) -> Result<(), MeNotifyError> {
        self.program().call(&self.ctx().to_lua(self.program().lua())?)
    }

    /// Calls one `loop(ctx)` entrypoint invocation.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the Lua function completed successfully.
    pub fn run_loop(&self) -> Result<(), MeNotifyError> {
        self.program().call(&self.ctx().to_lua(self.program().lua())?)
    }

    /// Calls one `tick(ctx)` entrypoint invocation with scoped `ctx.emit(...)`.
    ///
    /// # Arguments
    ///
    /// * `emit` - Sensor event sink.
    /// * `builder` - Event envelope builder.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the Lua function completed successfully.
    pub fn run_tick_with_emit(
        &self,
        emit: &(dyn Fn(serde_json::Value) + Send + Sync),
        builder: &MeNotifyEventBuilder,
    ) -> Result<(), MeNotifyError> {
        self.program().lua().scope(|scope| {
            self.program()
                .call(
                    &self
                        .ctx()
                        .to_lua_scoped(self.program().lua(), scope, emit, builder)
                        .map_err(|err| mlua::Error::runtime(err.to_string()))?,
                )
                .map_err(|err| mlua::Error::runtime(err.to_string()))
        })?;
        Ok(())
    }

    /// Calls one `loop(ctx)` entrypoint invocation with scoped `ctx.emit(...)`.
    ///
    /// # Arguments
    ///
    /// * `emit` - Sensor event sink.
    /// * `builder` - Event envelope builder.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the Lua function completed successfully.
    pub fn run_loop_with_emit(
        &self,
        emit: &(dyn Fn(serde_json::Value) + Send + Sync),
        builder: &MeNotifyEventBuilder,
    ) -> Result<(), MeNotifyError> {
        self.program().lua().scope(|scope| {
            self.program()
                .call(
                    &self
                        .ctx()
                        .to_lua_scoped(self.program().lua(), scope, emit, builder)
                        .map_err(|err| mlua::Error::runtime(err.to_string()))?,
                )
                .map_err(|err| mlua::Error::runtime(err.to_string()))
        })?;
        Ok(())
    }
}
