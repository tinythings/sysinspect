use colored::Colorize;
use libcommon::SysinspectError;
use libsysinspect::cfg::mmconf::HopstartConfig;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::{net::TcpStream, process::Command, sync::Semaphore};

/// Shared semaphore capping concurrent hopstart SSH calls across all callers.
pub(crate) static HOPSTART_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

/// Return the shared hopstart semaphore, falling back to allow-1 if not initialised.
pub(crate) fn hopstart_semaphore() -> Arc<Semaphore> {
    HOPSTART_SEMAPHORE.get().cloned().unwrap_or_else(|| Arc::new(Semaphore::new(1)))
}

/// Initialise the shared hopstart semaphore from the configured batch size.
pub(crate) fn init_hopstart_semaphore(batch: usize) {
    HOPSTART_SEMAPHORE.set(Arc::new(Semaphore::new(batch.max(1)))).ok();
}

#[derive(Clone)]
pub(crate) struct HopStartTarget {
    host: String,
    root: String,
    user: String,
    bin: String,
    config: String,
}

impl HopStartTarget {
    pub(crate) fn new(host: String, root: String, user: String, bin: String, config: String) -> Self {
        Self { host, root, user, bin, config }
    }

    fn remote_command(&self) -> String {
        format!("{} -c {} --daemon", shell_quote(&self.bin), shell_quote(&self.config))
    }

    fn ssh_target(&self) -> String {
        format!("{}@{}", self.user, self.host)
    }

    pub(crate) fn log_issue(&self) {
        log::info!("Hop-start {} at {} as {}", self.host.yellow(), self.root.bright_white().bold(), self.user.bright_blue().bold());
    }

    pub(crate) async fn issue(&self) -> Result<(), SysinspectError> {
        if !is_port_available(&self.host, 22, Duration::from_secs(5)).await {
            return Err(SysinspectError::MasterGeneralError(format!("Hop-start aborted: host {} is unreachable via SSH", self.host)));
        }

        let status = Command::new("ssh").arg(self.ssh_target()).arg(self.remote_command()).status().await?;

        if status.success() {
            return Ok(());
        }

        Err(SysinspectError::MasterGeneralError(format!(
            "Hop-start failed for {} with exit status {}",
            self.host,
            status.code().map(|code| code.to_string()).unwrap_or_else(|| "signal".to_string())
        )))
    }
}

pub(crate) struct HopStarter {
    cfg: HopstartConfig,
}

impl HopStarter {
    pub(crate) fn new(cfg: HopstartConfig) -> Self {
        Self { cfg }
    }

    pub(crate) async fn issue(&self, targets: Vec<HopStartTarget>) -> Vec<String> {
        let limit = hopstart_semaphore();
        let total = targets.len();
        let mut tasks = Vec::with_capacity(total);

        for target in targets {
            tasks.push(tokio::spawn({
                let limit = Arc::clone(&limit);
                async move {
                    let _permit = limit.acquire_owned().await.ok()?;
                    target.log_issue();
                    Some(match target.issue().await {
                        Ok(()) => Ok(target.host),
                        Err(err) => {
                            log::error!("{err}");
                            Err(target.host)
                        }
                    })
                }
            }));
        }

        let mut failed = Vec::new();
        let mut ok = 0usize;
        for task in tasks {
            match task.await {
                Ok(Some(Ok(_))) => ok += 1,
                Ok(Some(Err(host))) => failed.push(host),
                _ => {}
            }
        }

        let fail_count = failed.len();
        if fail_count == 0 {
            log::info!("Hop-start complete: all {total} minion(s) started");
        } else {
            log::warn!("Hop-start finished: {ok} started, {fail_count} failed out of {total}");
        }

        failed
    }
}

async fn is_port_available(host: &str, port: u16, timeout: Duration) -> bool {
    let addr = format!("{host}:{port}");
    tokio::time::timeout(timeout, TcpStream::connect(&addr)).await.map(|r| r.is_ok()).unwrap_or(false)
}

pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod hopstart_ut {
    use super::HopStartTarget;

    #[test]
    fn remote_command_keeps_full_minion_invocation_in_one_shell_string() {
        assert_eq!(
            HopStartTarget::new(
                "host".to_string(),
                "/srv/sysinspect".to_string(),
                "bo".to_string(),
                "/srv/sysinspect/bin/sysminion".to_string(),
                "/srv/sysinspect/etc/sysinspect.conf".to_string()
            )
            .remote_command(),
            "'/srv/sysinspect/bin/sysminion' -c '/srv/sysinspect/etc/sysinspect.conf' --daemon"
        );
    }
}
