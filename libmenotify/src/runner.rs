use crate::{MeNotifyContext, MeNotifyEntrypoint, MeNotifyError, MeNotifyProgram};

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
}
