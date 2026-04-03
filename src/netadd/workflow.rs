use crate::netadd::{
    artifact::{MinionArtifact, MinionCatalogue, PlatformId},
    parser::{parse_request, resolve_plan},
    render::render_outcomes,
    types::{AddOutcome, AddPlan, AddRequest},
};
use crate::sshprobe::{
    detect::{ProbeInfo, ProbePathKind, SSHPlatformDetector},
    transport::{ElevationMode, RemoteCommand, SSHEndpoint, SSHSession, UploadRequest, shell_quote},
};
use clap::ArgMatches;
use libcommon::SysinspectError;
use libsysinspect::{
    cfg::mmconf::{CFG_MASTER_KEY_PUB, MasterConfig},
    console::{ConsoleOnlineMinionRow, ConsolePayload},
    rsa::keys::{from_pem, get_fingerprint},
};
use libsysproto::query::{SCHEME_COMMAND, commands::CLUSTER_ONLINE_MINIONS};
use std::{
    fs,
    path::Path,
    thread::sleep,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
struct SetupContext {
    repo_root: std::path::PathBuf,
    cfg: MasterConfig,
    master_addr: String,
    master_fp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteLayout {
    stage_bin: String,
    install_bin: String,
    config: String,
    dir: Option<String>,
    pidfile: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostSetup {
    host: crate::netadd::types::AddHost,
    info: ProbeInfo,
    art: MinionArtifact,
    layout: RemoteLayout,
}

/// Dedicated host-onboarding workflow entrypoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NetworkAddWorkflow {
    req: AddRequest,
}

impl NetworkAddWorkflow {
    /// Build one workflow from CLI matches.
    pub(crate) fn from_matches(am: &ArgMatches) -> Result<Self, SysinspectError> {
        Ok(Self { req: parse_request(am)? })
    }

    /// Validate and resolve the host batch.
    pub(crate) fn plan(&self) -> Result<AddPlan, SysinspectError> {
        resolve_plan(&self.req)
    }

    /// Produce the current operator-facing console view.
    pub(crate) fn render(&self) -> Result<String, SysinspectError> {
        Ok(render_outcomes(
            &self.plan()?.items.into_iter().map(|host| AddOutcome { detail: "validated".to_string(), host, state: "planned" }).collect::<Vec<_>>(),
        ))
    }

    /// Probe every planned host and render the discovered target details.
    pub(crate) fn probe_render(&self) -> Result<String, SysinspectError> {
        let mut rows = Vec::new();
        for host in self.plan()?.items {
            let mut det = SSHPlatformDetector::new(&host.host).set_user(&host.user).check_writable(true);
            if let Some(path) = &host.path {
                det = det.set_destination(path);
            }
            match det.info() {
                Ok(info) => rows.push(AddOutcome { detail: info.summary(), host, state: "probed" }),
                Err(err) => rows.push(AddOutcome { detail: err.to_string(), host, state: "error" }),
            }
        }
        Ok(render_outcomes(&rows))
    }

    /// Select one local sysminion artefact for a probed target.
    pub(crate) fn select_artifact(&self, repo_root: &Path, info: &ProbeInfo) -> Result<MinionArtifact, SysinspectError> {
        MinionCatalogue::open(repo_root)?.select(&PlatformId::from_probe(info)?)
    }

    /// Probe, upload, set up, register, and start every planned host.
    pub(crate) fn setup_render(&self, cfg: &MasterConfig) -> Result<String, SysinspectError> {
        let ctx = SetupContext::from_cfg(cfg)?;
        let mut rows = Vec::new();
        for host in self.plan()?.items {
            match self.setup_host(&ctx, host.clone()) {
                Ok(detail) => rows.push(AddOutcome { detail, host, state: "setup" }),
                Err(err) => rows.push(AddOutcome { detail: err.to_string(), host, state: "error" }),
            }
        }
        Ok(render_outcomes(&rows))
    }

    fn setup_host(&self, ctx: &SetupContext, host: crate::netadd::types::AddHost) -> Result<String, SysinspectError> {
        let info = self.probe_host(&host)?;
        let art = self.select_artifact(&ctx.repo_root, &info)?;
        let layout = RemoteLayout::from_probe(&host, &info)?;
        let hs = HostSetup { host, info, art, layout };
        hs.run(ctx)
    }

    fn probe_host(&self, host: &crate::netadd::types::AddHost) -> Result<ProbeInfo, SysinspectError> {
        let mut det = SSHPlatformDetector::new(&host.host).set_user(&host.user).check_writable(true);
        if let Some(path) = &host.path {
            det = det.set_destination(path);
        }
        det.info()
    }
}

impl SetupContext {
    fn from_cfg(cfg: &MasterConfig) -> Result<Self, SysinspectError> {
        let master_addr = cfg.bind_addr();
        let host = master_addr.split(':').next().unwrap_or_default();
        if matches!(host, "" | "0.0.0.0" | "::" | "[::]") {
            return Err(SysinspectError::ConfigError(format!("Master bind address {} is not a reachable remote address for auto-add", master_addr)));
        }
        let pem = fs::read_to_string(cfg.root_dir().join(CFG_MASTER_KEY_PUB))?;
        let (_, pbk) = from_pem(None, Some(&pem))?;
        let pbk = pbk.ok_or_else(|| SysinspectError::ConfigError("Master RSA public key is missing from disk".to_string()))?;
        Ok(Self {
            repo_root: cfg.get_mod_repo_root(),
            cfg: cfg.clone(),
            master_addr,
            master_fp: get_fingerprint(&pbk).map_err(|err| SysinspectError::RSAError(err.to_string()))?,
        })
    }
}

impl RemoteLayout {
    fn from_probe(host: &crate::netadd::types::AddHost, info: &ProbeInfo) -> Result<Self, SysinspectError> {
        let stage_root = format!(
            "{}/sysinspect-autoadd-{}",
            info.tmp
                .as_deref()
                .ok_or_else(|| SysinspectError::MinionGeneralError(format!("Probe for {} did not return a temporary directory", host.host)))?
                .trim_end_matches('/'),
            host.host_norm.replace('.', "-")
        );
        let dir = match info.destination.kind {
            ProbePathKind::Custom => Some(
                info.destination
                    .resolved
                    .clone()
                    .ok_or_else(|| SysinspectError::MinionGeneralError(format!("Probe for {} did not resolve the destination path", host.host)))?,
            ),
            ProbePathKind::System => None,
        };
        let pidfile = dir.as_deref().map(|v| format!("{v}/run/sysinspect.pid")).unwrap_or_else(|| "/var/run/sysinspect.pid".to_string());
        Ok(Self {
            stage_bin: format!("{stage_root}/sysminion"),
            install_bin: dir.as_deref().map(|v| format!("{v}/bin/sysminion")).unwrap_or_else(|| "/usr/bin/sysminion".to_string()),
            config: dir.as_deref().map(|v| format!("{v}/etc/sysinspect.conf")).unwrap_or_else(|| "/etc/sysinspect/sysinspect.conf".to_string()),
            dir,
            pidfile,
        })
    }
}

impl HostSetup {
    fn run(&self, ctx: &SetupContext) -> Result<String, SysinspectError> {
        let ssh = SSHSession::new(SSHEndpoint::new(&self.host.host, &self.host.user));
        ssh.upload(&UploadRequest::new(&self.art.path, &self.layout.stage_bin))?;
        let elevate = self.elevation()?;
        ssh.exec(&RemoteCommand::new(format!("chmod 0755 {}", shell_quote(&self.layout.stage_bin))))?;
        ssh.exec(&RemoteCommand::new(self.setup_cmd(ctx)).elevate(elevate))?;
        ssh.exec(&RemoteCommand::new(self.register_cmd(ctx)).elevate(elevate))?;
        ssh.exec(&RemoteCommand::new(self.start_cmd()).elevate(elevate))?;
        self.wait_runtime(&ssh, elevate)?;
        self.wait_attempt(&ssh, elevate)?;
        self.wait_online(ctx)?;
        Ok(format!("{} artefact={} daemon online", self.info.summary(), self.art.version))
    }

    fn elevation(&self) -> Result<ElevationMode, SysinspectError> {
        if self.info.privilege == crate::sshprobe::detect::PrivilegeMode::Root || self.info.destination.writable {
            return Ok(ElevationMode::None);
        }
        if self.info.has_sudo {
            return Ok(ElevationMode::Sudo);
        }
        Err(SysinspectError::MinionGeneralError(format!("Destination is not writable on {} and sudo is unavailable", self.host.host)))
    }

    fn setup_cmd(&self, ctx: &SetupContext) -> String {
        format!(
            "{} setup --with-default-config --master-addr {}{}",
            shell_quote(&self.layout.stage_bin),
            shell_quote(&ctx.master_addr),
            self.layout.dir.as_deref().map(|dir| format!(" --directory {}", shell_quote(dir))).unwrap_or_default()
        )
    }

    fn register_cmd(&self, ctx: &SetupContext) -> String {
        format!("{} -c {} --register {}", shell_quote(&self.layout.install_bin), shell_quote(&self.layout.config), shell_quote(&ctx.master_fp))
    }

    fn start_cmd(&self) -> String {
        format!("{} -c {} --daemon", shell_quote(&self.layout.install_bin), shell_quote(&self.layout.config))
    }

    fn wait_runtime(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let deadline = Instant::now() + Duration::from_secs(12);
        let cmd = format!("test -s {0} && pid=$(cat {0}) && kill -0 \"$pid\"", shell_quote(&self.layout.pidfile));
        while Instant::now() < deadline {
            if ssh.exec(&RemoteCommand::new(cmd.clone()).elevate(elevate)).is_ok() {
                return Ok(());
            }
            sleep(Duration::from_millis(500));
        }
        Err(SysinspectError::MinionGeneralError(format!(
            "sysminion daemon did not stay alive on {}; {}",
            self.host.host,
            self.log_snapshot(ssh, elevate)?
        )))
    }

    fn wait_attempt(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let deadline = Instant::now() + Duration::from_secs(15);
        let cmd = self.attempt_probe_cmd();
        while Instant::now() < deadline {
            let rsp = ssh.exec(&RemoteCommand::new(cmd.clone()).elevate(elevate));
            if rsp.as_ref().is_ok_and(|rsp| rsp.stdout.trim() == "yes") {
                return Ok(());
            }
            sleep(Duration::from_millis(500));
        }
        Err(SysinspectError::MinionGeneralError(format!(
            "sysminion did not log a registration/bootstrap attempt on {}; {}",
            self.host.host,
            self.log_snapshot(ssh, elevate)?
        )))
    }

    fn wait_online(&self, ctx: &SetupContext) -> Result<(), SysinspectError> {
        let deadline = Instant::now() + Duration::from_secs(20);
        let ssh = SSHSession::new(SSHEndpoint::new(&self.host.host, &self.host.user));
        let elevate = self.elevation()?;
        while Instant::now() < deadline {
            if self.online(ctx)? {
                return Ok(());
            }
            sleep(Duration::from_secs(1));
        }
        Err(SysinspectError::MinionGeneralError(format!(
            "sysminion started on {} but did not appear in network --online in time; {}",
            self.host.host,
            self.log_snapshot(&ssh, elevate)?
        )))
    }

    fn online(&self, ctx: &SetupContext) -> Result<bool, SysinspectError> {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| SysinspectError::DynError(Box::new(e)))?;
        let rsp = rt.block_on(crate::call_master_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_ONLINE_MINIONS}"), "*", None, None, None))?;
        let ConsolePayload::OnlineMinions { rows } = rsp.payload else {
            return Err(SysinspectError::MasterGeneralError("Master returned an unexpected payload while checking online minions".to_string()));
        };
        Ok(rows.into_iter().any(|row| row.alive && self.matches_online_row(&row)))
    }

    fn matches_online_row(&self, row: &ConsoleOnlineMinionRow) -> bool {
        let host = self.host.host_norm.as_str();
        [row.fqdn.as_str(), row.hostname.as_str(), row.ip.as_str()]
            .into_iter()
            .map(normalise_remote_name)
            .any(|name| !name.is_empty() && name == host)
    }

    fn attempt_probe_cmd(&self) -> String {
        format!(
            "found=no; for p in {}; do if [ -f \"$p\" ] && grep -Eq {} \"$p\"; then found=yes; break; fi; done; printf '%s' \"$found\"",
            self.log_candidates_expr(),
            shell_quote("Registration request to|Ehlo on|Secure session established|Unable to bootstrap secure transport")
        )
    }

    fn log_snapshot(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<String, SysinspectError> {
        let rsp = ssh.exec(&RemoteCommand::new(self.log_snapshot_cmd()).elevate(elevate))?;
        let out = rsp.stdout.trim();
        Ok(if out.is_empty() { "no remote logs found".to_string() } else { format!("remote logs: {out}") })
    }

    fn log_snapshot_cmd(&self) -> String {
        format!(
            "for p in {}; do if [ -f \"$p\" ]; then printf '%s: ' \"$p\"; tail -n 20 \"$p\" 2>/dev/null | tr '\\n' ' '; fi; done",
            self.log_candidates_expr()
        )
    }

    fn log_candidates_expr(&self) -> String {
        "/var/log/sysminion.standard.log /var/log/sysminion.errors.log \"$HOME/.local/state/sysminion.standard.log\" \"$HOME/.local/state/sysminion.errors.log\" /tmp/sysminion.standard.log /tmp/sysminion.errors.log".to_string()
    }
}

fn normalise_remote_name(name: &str) -> String {
    name.trim().trim_end_matches('.').to_ascii_lowercase()
}
