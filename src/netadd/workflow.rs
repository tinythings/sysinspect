use crate::netadd::{
    artifact::{MinionArtifact, MinionCatalogue, PlatformId},
    parser::{parse_request, resolve_plan},
    render::render_outcomes,
    types::{AddOutcome, AddPlan, AddRequest},
};
use crate::sshprobe::{
    detect::{ProbeInfo, ProbePathKind, SSHPlatformDetector},
    transport::{ElevationMode, RemoteCommand, SSHEndpoint, SSHSession, UploadMethod, UploadRequest, shell_quote},
};
use clap::ArgMatches;
use libcommon::SysinspectError;
use libsysinspect::{
    cfg::mmconf::{CFG_MASTER_KEY_PUB, MasterConfig},
    console::{ConsoleMinionInfoRow, ConsoleOnlineMinionRow, ConsolePayload, ConsoleTransportStatusRow},
    rsa::keys::{from_pem, get_fingerprint},
};
use libsysproto::query::{
    SCHEME_COMMAND,
    commands::{CLUSTER_MINION_INFO, CLUSTER_ONLINE_MINIONS, CLUSTER_REMOVE_MINION, CLUSTER_TRANSPORT_STATUS},
};
use serde_json::json;
use std::{
    fs,
    net::UdpSocket,
    path::Path,
    thread::sleep,
    time::{Duration, Instant},
};
use tokio::{runtime::Handle, task::block_in_place};

#[derive(Debug, Clone)]
struct SetupContext {
    repo_root: std::path::PathBuf,
    cfg: MasterConfig,
    master_fp: String,
    master_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteLayout {
    stage_bin: String,
    install_bin: String,
    config: String,
    dir: Option<String>,
    machine_id: String,
    pidfile: String,
    log_std: String,
    log_err: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostSetup {
    host: crate::netadd::types::AddHost,
    info: ProbeInfo,
    art: MinionArtifact,
    layout: RemoteLayout,
    minion_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Readiness {
    online: bool,
    traits: bool,
    transport: bool,
    sensors: bool,
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

    /// Select one local sysminion artefact for a probed target.
    pub(crate) fn select_artifact(&self, repo_root: &Path, info: &ProbeInfo) -> Result<MinionArtifact, SysinspectError> {
        MinionCatalogue::open(repo_root)?.select(&PlatformId::from_probe(info)?)
    }

    /// Probe, upload, set up, register, and start every planned host.
    pub(crate) fn setup_render(&self, cfg: &MasterConfig) -> Result<String, SysinspectError> {
        let ctx = SetupContext::from_cfg(cfg)?;
        let mut rows = Vec::new();
        for host in self.plan()?.items {
            log::info!("Auto-add: onboarding {} as {}", host.host, host.user);
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
        let hs = HostSetup { host, info, art, layout, minion_id: None };
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
        let pem = fs::read_to_string(cfg.root_dir().join(CFG_MASTER_KEY_PUB))?;
        let (_, pbk) = from_pem(None, Some(&pem))?;
        let pbk = pbk.ok_or_else(|| SysinspectError::ConfigError("Master RSA public key is missing from disk".to_string()))?;
        Ok(Self {
            repo_root: cfg.get_mod_repo_root(),
            cfg: cfg.clone(),
            master_fp: get_fingerprint(&pbk).map_err(|err| SysinspectError::RSAError(err.to_string()))?,
            master_port: cfg
                .bind_addr()
                .rsplit(':')
                .next()
                .and_then(|v| v.parse::<u16>().ok())
                .ok_or_else(|| SysinspectError::ConfigError(format!("Invalid master bind address: {}", cfg.bind_addr())))?,
        })
    }

    fn master_addr_for(&self, host: &str) -> Result<String, SysinspectError> {
        let bind = self.cfg.bind_addr();
        let ip = bind.split(':').next().unwrap_or_default();
        if !matches!(ip, "" | "0.0.0.0" | "::" | "[::]") {
            return Ok(bind);
        }

        let sock = UdpSocket::bind("0.0.0.0:0")?;
        sock.connect((host, 22)).map_err(|err| {
            SysinspectError::ConfigError(format!("Unable to derive a reachable master address for {host} from wildcard bind {}: {err}", bind))
        })?;
        Ok(format!("{}:{}", sock.local_addr()?.ip(), self.master_port))
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
            ProbePathKind::Home | ProbePathKind::Custom => Some(
                info.destination
                    .resolved
                    .clone()
                    .ok_or_else(|| SysinspectError::MinionGeneralError(format!("Probe for {} did not resolve the destination path", host.host)))?,
            ),
            ProbePathKind::System => None,
        };
        let pidfile = dir.as_deref().map(|v| format!("{v}/run/sysinspect.pid")).unwrap_or_else(|| "/var/run/sysinspect.pid".to_string());
        let log_std =
            dir.as_deref().map(|v| format!("{v}/tmp/sysminion.standard.log")).unwrap_or_else(|| "/var/log/sysminion.standard.log".to_string());
        let log_err = dir.as_deref().map(|v| format!("{v}/tmp/sysminion.errors.log")).unwrap_or_else(|| "/var/log/sysminion.errors.log".to_string());
        Ok(Self {
            stage_bin: format!("{stage_root}/sysminion"),
            install_bin: dir.as_deref().map(|v| format!("{v}/bin/sysminion")).unwrap_or_else(|| "/usr/bin/sysminion".to_string()),
            config: dir.as_deref().map(|v| format!("{v}/etc/sysinspect.conf")).unwrap_or_else(|| "/etc/sysinspect/sysinspect.conf".to_string()),
            dir,
            machine_id: "/etc/machine-id".to_string(),
            pidfile,
            log_std,
            log_err,
        })
    }
}

impl HostSetup {
    fn run(&self, ctx: &SetupContext) -> Result<String, SysinspectError> {
        let ssh = SSHSession::new(SSHEndpoint::new(&self.host.host, &self.host.user));
        log::info!("Auto-add {}: upload {}", self.host.host, self.art.path.display());
        ssh.upload(&UploadRequest::new(&self.art.path, &self.layout.stage_bin).methods(vec![UploadMethod::Stream, UploadMethod::Scp]))?;
        let elevate = self.elevation()?;
        ssh.exec(&RemoteCommand::new(format!("chmod 0755 {}", shell_quote(&self.layout.stage_bin))))?;
        self.verify_stage_bin(&ssh)?;
        log::info!("Auto-add {}: setup {}", self.host.host, self.layout.config);
        ssh.exec(&RemoteCommand::new(self.setup_cmd(ctx)?).elevate(elevate))?;
        let mut this = self.clone();
        this.prepare_runtime(&ssh, elevate)?;
        this.minion_id = this.read_minion_id(&ssh)?;
        log::info!("Auto-add {}: register", self.host.host);
        this.register(ctx, &ssh, elevate)?;
        log::info!("Auto-add {}: start daemon", self.host.host);
        ssh.exec(&RemoteCommand::new(this.start_cmd()).elevate(elevate))?;
        log::info!("Auto-add {}: wait for daemon pid", self.host.host);
        this.wait_runtime(&ssh, elevate)?;
        log::info!("Auto-add {}: wait for bootstrap log", self.host.host);
        this.wait_attempt(&ssh, elevate)?;
        log::info!("Auto-add {}: wait for master readiness", self.host.host);
        this.wait_ready(ctx, &ssh, elevate)?;
        Ok(format!("{} artefact={} daemon online", this.info.summary(), this.art.version))
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

    fn setup_cmd(&self, ctx: &SetupContext) -> Result<String, SysinspectError> {
        let master_addr = ctx.master_addr_for(&self.host.host)?;
        Ok(format!(
            "{} setup --with-default-config --master-addr {}{}",
            shell_quote(&self.layout.stage_bin),
            shell_quote(&master_addr),
            self.layout.dir.as_deref().map(|dir| format!(" --directory {}", shell_quote(dir))).unwrap_or_default()
        ))
    }

    fn register_cmd(&self, ctx: &SetupContext) -> String {
        format!("{} -c {} --register {}", shell_quote(&self.layout.install_bin), shell_quote(&self.layout.config), shell_quote(&ctx.master_fp))
    }

    fn register(&self, ctx: &SetupContext, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let cmd = RemoteCommand::new(self.register_cmd(ctx)).elevate(elevate);
        match ssh.exec(&cmd) {
            Ok(rsp) => {
                if rsp.stdout.contains("Registration key mismatch for ")
                    && let Some(mid) = registration_mismatch_id(&rsp.stdout)
                {
                    log::warn!("Auto-add {}: removing stale master registration for {}", self.host.host, mid);
                    self.unregister_stale(ctx, &mid)?;
                    return ssh.exec(&cmd).map(|_| ());
                }
                Ok(())
            }
            Err(err) => {
                let msg = err.to_string();
                let Some(mid) = registration_mismatch_id(&msg) else {
                    return Err(err);
                };
                log::warn!("Auto-add {}: removing stale master registration for {}", self.host.host, mid);
                self.unregister_stale(ctx, &mid)?;
                ssh.exec(&cmd).map(|_| ())
            }
        }
    }

    fn start_cmd(&self) -> String {
        format!(
            "trap '' HUP; {} -c {} --start </dev/null >>{} 2>>{} & printf '%s\\n' \"$!\" > {}",
            shell_quote(&self.layout.install_bin),
            shell_quote(&self.layout.config),
            shell_quote(&self.layout.log_std),
            shell_quote(&self.layout.log_err),
            shell_quote(&self.layout.pidfile)
        )
    }

    fn prepare_runtime(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let stop = format!(
            "if [ -s {0} ]; then pid=$(cat {0} 2>/dev/null || true); if [ -n \"$pid\" ] && kill -0 \"$pid\" 2>/dev/null; then kill \"$pid\" 2>/dev/null || true; sleep 1; fi; fi; rm -f {0} {1} {2}",
            shell_quote(&self.layout.pidfile),
            shell_quote(&self.layout.log_std),
            shell_quote(&self.layout.log_err)
        );
        ssh.exec(&RemoteCommand::new(stop).elevate(elevate)).map(|_| ())
    }

    fn read_minion_id(&self, ssh: &SSHSession) -> Result<Option<String>, SysinspectError> {
        let rsp = ssh.exec(&RemoteCommand::new(format!("cat {} 2>/dev/null || true", shell_quote(&self.layout.machine_id))))?;
        let mid = rsp.stdout.trim();
        Ok((!mid.is_empty()).then(|| mid.to_string()))
    }

    fn verify_stage_bin(&self, ssh: &SSHSession) -> Result<(), SysinspectError> {
        let chk = format!("test -s {} && {} --version >/dev/null 2>&1", shell_quote(&self.layout.stage_bin), shell_quote(&self.layout.stage_bin));
        match ssh.exec(&RemoteCommand::new(chk)) {
            Ok(_) => Ok(()),
            Err(_) => Err(SysinspectError::MinionGeneralError(format!(
                "Uploaded sysminion is not runnable on {}; {}",
                self.host.host,
                self.stage_snapshot(ssh)?
            ))),
        }
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
            if let Ok(rsp) = rsp {
                match rsp.stdout.trim() {
                    "yes" => return Ok(()),
                    "fail" => {
                        return Err(SysinspectError::MinionGeneralError(format!(
                            "sysminion failed during bootstrap on {}; {}",
                            self.host.host,
                            self.log_snapshot(ssh, elevate)?
                        )));
                    }
                    _ => {}
                }
            }
            sleep(Duration::from_millis(500));
        }
        Err(SysinspectError::MinionGeneralError(format!(
            "sysminion did not log a registration/bootstrap attempt on {}; {}",
            self.host.host,
            self.log_snapshot(ssh, elevate)?
        )))
    }

    fn wait_ready(&self, ctx: &SetupContext, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let deadline = Instant::now() + Duration::from_secs(25);
        while Instant::now() < deadline {
            if self.readiness(ctx, ssh, elevate)?.ready() {
                return Ok(());
            }
            sleep(Duration::from_secs(1));
        }
        Err(SysinspectError::MinionGeneralError(format!(
            "sysminion started on {} but did not reach full master readiness in time; {} ({})",
            self.host.host,
            self.readiness(ctx, ssh, elevate)?.detail(),
            self.log_snapshot(ssh, elevate)?
        )))
    }

    fn readiness(&self, ctx: &SetupContext, ssh: &SSHSession, elevate: ElevationMode) -> Result<Readiness, SysinspectError> {
        Ok(Readiness {
            online: self.online(ctx)?,
            traits: self.has_traits(ctx)?,
            transport: self.has_transport(ctx)?,
            sensors: self.sensors_ready(ssh, elevate)?,
        })
    }

    fn online(&self, ctx: &SetupContext) -> Result<bool, SysinspectError> {
        let rsp = call_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_ONLINE_MINIONS}"), "*", None, None)?;
        let ConsolePayload::OnlineMinions { rows } = rsp.payload else {
            return Err(SysinspectError::MasterGeneralError("Master returned an unexpected payload while checking online minions".to_string()));
        };
        Ok(rows.into_iter().any(|row| row.alive && self.matches_online_row(&row)))
    }

    fn has_traits(&self, ctx: &SetupContext) -> Result<bool, SysinspectError> {
        let rsp = call_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MINION_INFO}"), "*", self.minion_id.as_deref(), None)?;
        let ConsolePayload::MinionInfo { rows } = rsp.payload else {
            return Err(SysinspectError::MasterGeneralError("Master returned an unexpected payload while checking minion traits".to_string()));
        };
        Ok(rows_have_traits(&rows))
    }

    fn has_transport(&self, ctx: &SetupContext) -> Result<bool, SysinspectError> {
        let rsp = call_console(
            &ctx.cfg,
            &format!("{SCHEME_COMMAND}{CLUSTER_TRANSPORT_STATUS}"),
            "*",
            self.minion_id.as_deref(),
            Some(&json!({ "filter": "all" }).to_string()),
        )?;
        let ConsolePayload::TransportStatus { rows } = rsp.payload else {
            return Err(SysinspectError::MasterGeneralError("Master returned an unexpected payload while checking transport state".to_string()));
        };
        Ok(rows.into_iter().any(|row| self.matches_transport_row(&row) && row.active_key_id.is_some() && row.last_handshake_at.is_some()))
    }

    fn sensors_ready(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<bool, SysinspectError> {
        ssh.exec(&RemoteCommand::new(self.sensors_probe_cmd()).elevate(elevate)).map(|rsp| rsp.stdout.trim() == "yes")
    }

    fn matches_online_row(&self, row: &ConsoleOnlineMinionRow) -> bool {
        if self.minion_id.as_deref().is_some_and(|mid| row.minion_id == mid) {
            return true;
        }
        let host = self.host.host_norm.as_str();
        [row.fqdn.as_str(), row.hostname.as_str(), row.ip.as_str()]
            .into_iter()
            .map(normalise_remote_name)
            .any(|name| !name.is_empty() && name == host)
    }

    fn matches_transport_row(&self, row: &ConsoleTransportStatusRow) -> bool {
        self.minion_id.as_deref().is_some_and(|mid| row.minion_id == mid)
    }

    fn attempt_probe_cmd(&self) -> String {
        format!(
            "state=no; for p in {}; do if [ -f \"$p\" ] && grep -Eq {} \"$p\"; then state=fail; break; fi; if [ -f \"$p\" ] && grep -Eq {} \"$p\"; then state=yes; break; fi; done; printf '%s' \"$state\"",
            self.log_candidates_expr(),
            shell_quote("Minion encountered an error:|Unable to bootstrap secure transport|failed to lookup address information"),
            shell_quote("Registration request to|Ehlo on|Secure session established|Unable to bootstrap secure transport")
        )
    }

    fn sensors_probe_cmd(&self) -> String {
        format!(
            "state=no; for p in {}; do if [ -f \"$p\" ] && grep -Eq {} \"$p\"; then state=yes; break; fi; done; printf '%s' \"$state\"",
            self.log_candidates_expr(),
            shell_quote("Sending sensors sync callback for cycle|Received sensors sync response from master")
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

    fn stage_snapshot(&self, ssh: &SSHSession) -> Result<String, SysinspectError> {
        let rsp = ssh.exec(&RemoteCommand::new(format!(
            "p={0}; d=$(dirname \"$p\"); ls -ld \"$d\" \"$p\" 2>/dev/null || true",
            shell_quote(&self.layout.stage_bin)
        )))?;
        let out = rsp.stdout.trim();
        Ok(if out.is_empty() { "uploaded file not visible on remote host".to_string() } else { out.to_string() })
    }

    fn log_candidates_expr(&self) -> String {
        format!(
            "{} {} /var/log/sysminion.standard.log /var/log/sysminion.errors.log \"$HOME/.local/state/sysminion.standard.log\" \"$HOME/.local/state/sysminion.errors.log\" /tmp/sysminion.standard.log /tmp/sysminion.errors.log",
            shell_quote(&self.layout.log_std),
            shell_quote(&self.layout.log_err)
        )
    }

    fn unregister_stale(&self, ctx: &SetupContext, mid: &str) -> Result<(), SysinspectError> {
        let rsp = call_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_REMOVE_MINION}"), "", Some(mid), None)?;
        match rsp.payload {
            ConsolePayload::Ack { .. } => Ok(()),
            payload => Err(SysinspectError::MasterGeneralError(format!("Unable to remove stale minion {mid} before retry: {payload:?}"))),
        }
    }
}

fn normalise_remote_name(name: &str) -> String {
    name.trim().trim_end_matches('.').to_ascii_lowercase()
}

pub(crate) fn registration_mismatch_id(msg: &str) -> Option<String> {
    msg.split("Registration key mismatch for ")
        .nth(1)
        .and_then(|s| s.split(':').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn call_console(
    cfg: &MasterConfig, model: &str, query: &str, mid: Option<&str>, context: Option<&String>,
) -> Result<libsysinspect::console::ConsoleResponse, SysinspectError> {
    if let Ok(handle) = Handle::try_current() {
        return block_in_place(|| handle.block_on(crate::call_master_console(cfg, model, query, None, mid, context)));
    }

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| SysinspectError::DynError(Box::new(e)))?
        .block_on(crate::call_master_console(cfg, model, query, None, mid, context))
}

impl Readiness {
    fn ready(&self) -> bool {
        self.online && self.traits && self.transport && self.sensors
    }

    fn detail(&self) -> String {
        format!("readiness online={}, traits={}, transport={}, sensors={}", self.online, self.traits, self.transport, self.sensors)
    }
}

pub(crate) fn rows_have_traits(rows: &[ConsoleMinionInfoRow]) -> bool {
    rows.iter().any(|row| row.key == "minion.online" && row.value.as_bool() == Some(true))
        && rows.iter().any(|row| row.key == "system.id")
        && rows.iter().any(|row| row.key == "system.hostname")
}
