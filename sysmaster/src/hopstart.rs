use colored::Colorize;
use libcommon::SysinspectError;
use libsysinspect::cfg::mmconf::HopstartConfig;
use tokio::{process::Command, sync::Semaphore};

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

    fn log_issue(&self) {
        log::info!("Hop-start {} at {} as {}", self.host.yellow(), self.root.bright_white().bold(), self.user.bright_blue().bold());
    }

    async fn issue(&self) -> Result<(), SysinspectError> {
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

    pub(crate) async fn issue(&self, targets: Vec<HopStartTarget>) {
        let limit = std::sync::Arc::new(Semaphore::new(self.cfg.batch().max(1)));
        let mut tasks = Vec::with_capacity(targets.len());

        for target in targets {
            tasks.push(tokio::spawn({
                let limit = std::sync::Arc::clone(&limit);
                async move {
                    if let Ok(_permit) = limit.acquire_owned().await {
                        target.log_issue();
                        if let Err(err) = target.issue().await {
                            log::error!("{err}");
                        }
                    }
                }
            }));
        }

        for task in tasks {
            if let Err(err) = task.await {
                log::error!("Hop-start task failed: {err}");
            }
        }
    }
}

fn shell_quote(value: &str) -> String {
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
