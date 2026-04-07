use crate::netadd::{
    artifact::{MinionArtifact, MinionCatalogue, PlatformId},
    parser::{parse_request, resolve_plan},
    render::{render_outcomes, render_results},
    types::{AddOutcome, AddPlan, AddRequest, AddStatus, HostOp},
};
use crate::sshprobe::{
    detect::{ProbeInfo, ProbePathKind, SSHPlatformDetector},
    transport::{ElevationMode, RemoteCommand, SSHEndpoint, SSHSession, UploadMethod, UploadRequest, shell_quote},
};
use clap::ArgMatches;
use libcommon::SysinspectError;
use libmodpak::compare_versions;
use libsetup::local_marker::LocalMarker;
use libsysinspect::{
    cfg::mmconf::{CFG_MASTER_KEY_PUB, DEFAULT_MINION_LOG_ERR, DEFAULT_MINION_LOG_STD, MasterConfig, MinionConfig},
    console::{ConsoleMinionInfoRow, ConsoleOnlineMinionRow, ConsolePayload},
    rsa::keys::{from_pem, get_fingerprint},
};
use libsysproto::query::{
    SCHEME_COMMAND,
    commands::{CLUSTER_CMDB_UPSERT, CLUSTER_MINION_INFO, CLUSTER_ONLINE_MINIONS, CLUSTER_REMOVE_MINION, CLUSTER_SYNC, CLUSTER_TRANSPORT_STATUS},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
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
    root_dir: String,
    stage_bin: String,
    install_bin: String,
    config: String,
    local_marker: String,
    dir: Option<String>,
    traits_dir: String,
    onboarding_traits: String,
    machine_id: String,
    pidfile: String,
    log_std: String,
    log_err: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostSetup {
    target: ProbedHost,
    art: MinionArtifact,
    minion_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Readiness {
    online: bool,
    traits: bool,
    transport: bool,
    startup_sync: bool,
    sensors: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddFailureStage {
    Upload,
    Setup,
    Register,
    Start,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbedHost {
    host: crate::netadd::types::AddHost,
    info: ProbeInfo,
    layout: RemoteLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagedInstallState {
    Absent,
    Managed,
    NotManaged,
    Broken,
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
            &self
                .plan()?
                .items
                .into_iter()
                .map(|host| AddOutcome {
                    display_path: host.path.clone().unwrap_or_else(|| "<probe>".to_string()),
                    platform: "-".to_string(),
                    status: AddStatus::Pending,
                    host,
                })
                .collect::<Vec<_>>(),
            self.req.op,
        ))
    }

    /// Select one local sysminion artefact for a probed target.
    pub(crate) fn select_artifact(&self, repo_root: &Path, info: &ProbeInfo) -> Result<MinionArtifact, SysinspectError> {
        MinionCatalogue::open(repo_root)?.select(&PlatformId::from_probe(info)?)
    }

    /// Probe and apply the requested host lifecycle change to every planned host.
    pub(crate) fn run_render(&self, cfg: &MasterConfig) -> Result<String, SysinspectError> {
        let ctx = SetupContext::from_cfg(cfg)?;
        let mut rows = Vec::new();
        for host in self.plan()?.items {
            log::info!("{} {} as {}", self.req.op.progress_label(), host.host, host.user);
            match self.probe_host(&host) {
                Ok(info) => match ProbedHost::new(host.clone(), info) {
                    Ok(target) => match self.run_host(&ctx, &target) {
                        Ok(status) => rows.push(target.outcome(status)),
                        Err(err) => {
                            log::error!("{} {} failed: {}", self.req.op.progress_label(), target.host.host, err);
                            rows.push(target.outcome(AddStatus::Failed));
                        }
                    },
                    Err(err) => {
                        log::error!("{} {} failed: {}", self.req.op.progress_label(), host.host, err);
                        rows.push(AddOutcome { display_path: "-".to_string(), platform: "-".to_string(), status: AddStatus::Failed, host });
                    }
                },
                Err(err) => {
                    log::error!("{} {} failed: {}", self.req.op.progress_label(), host.host, err);
                    rows.push(AddOutcome {
                        display_path: host.path.clone().unwrap_or_else(|| "-".to_string()),
                        platform: "-".to_string(),
                        status: if self.req.op == HostOp::Remove && self.req.force {
                            self.force_remove_host(&ctx, &host).unwrap_or(AddStatus::Failed)
                        } else {
                            AddStatus::Failed
                        },
                        host,
                    });
                }
            }
        }
        Ok(render_results(&rows, self.req.op))
    }

    fn run_host(&self, ctx: &SetupContext, target: &ProbedHost) -> Result<AddStatus, SysinspectError> {
        if self.req.op == HostOp::Add {
            match self.managed_add_state(target)? {
                ManagedInstallState::Absent => {}
                ManagedInstallState::Managed => {
                    if !self.req.force {
                        log::info!("Auto-add {}: already added", target.host.host);
                        return Ok(AddStatus::AlreadyAdded);
                    }

                    log::warn!("Auto-add {}: --force requested, removing existing managed install first", target.host.host);
                    self.remove_host(ctx, target)?;
                }
                ManagedInstallState::NotManaged => {
                    log::warn!(
                        "Auto-add {}: installation already exists at {} but ownership is not recognized by sysinspect; remove it first and retry",
                        target.host.host,
                        target.layout.root_dir
                    );
                    return Ok(AddStatus::NotManaged);
                }
                ManagedInstallState::Broken => {
                    if !self.req.force {
                        log::warn!(
                            "Auto-add {}: destination {} is missing but managed remnants still exist; rerun with --force to repair",
                            target.host.host,
                            target.layout.root_dir
                        );
                        return Ok(AddStatus::NotManaged);
                    }

                    log::warn!(
                        "Auto-add {}: destination {} is missing but managed remnants still exist; removing broken state first",
                        target.host.host,
                        target.layout.root_dir
                    );
                    self.remove_host(ctx, target)?;
                }
            }
            return HostSetup { target: target.clone(), art: self.select_artifact(&ctx.repo_root, &target.info)?, minion_id: None }
                .run(ctx)
                .map(|_| AddStatus::Online);
        }
        if self.req.op == HostOp::Upgrade {
            return self.upgrade_host(ctx, target);
        }
        self.remove_host(ctx, target)
    }

    fn probe_host(&self, host: &crate::netadd::types::AddHost) -> Result<ProbeInfo, SysinspectError> {
        host.path
            .as_deref()
            .map_or_else(
                || SSHPlatformDetector::new(&host.host).set_user(&host.user).check_writable(true),
                |path| SSHPlatformDetector::new(&host.host).set_user(&host.user).check_writable(true).set_destination(path),
            )
            .info()
    }

    fn managed_add_state(&self, target: &ProbedHost) -> Result<ManagedInstallState, SysinspectError> {
        let ssh = target.ssh();
        let elevate = target.elevation()?;
        let destination_exists = ssh
            .exec(
                &RemoteCommand::new(format!("state=no; [ -e {} ] && state=yes; printf '%s' \"$state\"", shell_quote(&target.layout.root_dir)))
                    .elevate(elevate),
            )?
            .stdout
            .trim()
            == "yes";
        let marker = if destination_exists {
            ssh.exec(&RemoteCommand::new(format!("cat {} 2>/dev/null || true", shell_quote(&target.layout.local_marker))).elevate(elevate))?.stdout
        } else {
            String::new()
        };
        let orphaned_traces = if destination_exists {
            false
        } else {
            ssh.exec(
                &RemoteCommand::new(format!(
                    "state=no; for p in {} {} {}; do [ -e \"$p\" ] && state=yes && break; done; printf '%s' \"$state\"",
                    shell_quote(&target.layout.install_bin),
                    shell_quote(&target.layout.config),
                    shell_quote(&target.layout.local_marker)
                ))
                .elevate(elevate),
            )?
            .stdout
            .trim()
                == "yes"
        };

        Ok(classify_destination_state(&target.layout.root_dir, destination_exists, &marker, orphaned_traces))
    }
}

impl ProbedHost {
    fn new(host: crate::netadd::types::AddHost, info: ProbeInfo) -> Result<Self, SysinspectError> {
        Ok(Self { layout: RemoteLayout::from_probe(&host, &info)?, host, info })
    }

    fn outcome(&self, status: AddStatus) -> AddOutcome {
        AddOutcome { display_path: self.layout.root_dir.clone(), platform: self.info.os_arch(), status, host: self.host.clone() }
    }

    fn ssh(&self) -> SSHSession {
        SSHSession::new(SSHEndpoint::new(&self.host.host, &self.host.user))
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
}

impl SetupContext {
    fn from_cfg(cfg: &MasterConfig) -> Result<Self, SysinspectError> {
        Ok(Self {
            repo_root: cfg.get_mod_repo_root(),
            cfg: cfg.clone(),
            master_fp: get_fingerprint(
                &from_pem(None, Some(&fs::read_to_string(cfg.root_dir().join(CFG_MASTER_KEY_PUB))?))?
                    .1
                    .ok_or_else(|| SysinspectError::ConfigError("Master RSA public key is missing from disk".to_string()))?,
            )
            .map_err(|err| SysinspectError::RSAError(err.to_string()))?,
            master_port: cfg
                .bind_addr()
                .rsplit(':')
                .next()
                .and_then(|v| v.parse::<u16>().ok())
                .ok_or_else(|| SysinspectError::ConfigError(format!("Invalid master bind address: {}", cfg.bind_addr())))?,
        })
    }

    fn master_addr_for(&self, host: &str) -> Result<String, SysinspectError> {
        if !matches!(self.cfg.bind_addr().split(':').next().unwrap_or_default(), "" | "0.0.0.0" | "::" | "[::]") {
            return Ok(self.cfg.bind_addr());
        }
        UdpSocket::bind("0.0.0.0:0")
            .and_then(|sock| {
                sock.connect((host, 22))?;
                sock.local_addr().map(|addr| format!("{}:{}", addr.ip(), self.master_port))
            })
            .map_err(|err| {
                SysinspectError::ConfigError(format!(
                    "Unable to derive a reachable master address for {host} from wildcard bind {}: {err}",
                    self.cfg.bind_addr()
                ))
            })
    }
}

impl RemoteLayout {
    fn from_probe(host: &crate::netadd::types::AddHost, info: &ProbeInfo) -> Result<Self, SysinspectError> {
        let mut cfg = MinionConfig::default();
        if let Some(root) = match info.destination.kind {
            ProbePathKind::Home | ProbePathKind::Custom => Some(
                info.destination
                    .resolved
                    .clone()
                    .ok_or_else(|| SysinspectError::MinionGeneralError(format!("Probe for {} did not resolve the destination path", host.host)))?,
            ),
            ProbePathKind::System => None,
        }
        .as_deref()
        {
            cfg.set_root_dir(root);
        }
        Ok(Self {
            root_dir: cfg.root_dir().display().to_string(),
            stage_bin: format!(
                "{}/sysinspect-autoadd-{}/sysminion",
                info.tmp
                    .as_deref()
                    .ok_or_else(|| SysinspectError::MinionGeneralError(format!("Probe for {} did not return a temporary directory", host.host)))?
                    .trim_end_matches('/'),
                host.host_norm.replace('.', "-")
            ),
            install_bin: cfg.install_bin_path().display().to_string(),
            config: cfg.config_path().display().to_string(),
            local_marker: cfg.local_marker_path().display().to_string(),
            dir: match info.destination.kind {
                ProbePathKind::Home | ProbePathKind::Custom => {
                    Some(info.destination.resolved.clone().ok_or_else(|| {
                        SysinspectError::MinionGeneralError(format!("Probe for {} did not resolve the destination path", host.host))
                    })?)
                }
                ProbePathKind::System => None,
            },
            traits_dir: cfg.traits_dir().display().to_string(),
            onboarding_traits: format!("{}/autoadd.cfg", cfg.traits_dir().display()),
            machine_id: cfg.machine_id_path().display().to_string(),
            pidfile: cfg.managed_pidfile_path().display().to_string(),
            log_std: cfg.managed_logfile_std_path().display().to_string(),
            log_err: cfg.managed_logfile_err_path().display().to_string(),
        })
    }
}

impl HostSetup {
    fn run(&self, ctx: &SetupContext) -> Result<(), SysinspectError> {
        let ssh = self.target.ssh();
        let elevate = self.target.elevation()?;
        log::info!("Auto-add {}: upload {}", self.target.host.host, self.art.path.display());
        ssh.upload(&UploadRequest::new(&self.art.path, &self.target.layout.stage_bin).methods(vec![UploadMethod::Stream, UploadMethod::Scp]))
            .inspect_err(|err| self.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Upload, None, err))?;
        ssh.exec(&RemoteCommand::new(format!("chmod 0755 {}", shell_quote(&self.target.layout.stage_bin))))
            .inspect_err(|err| self.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Upload, None, err))?;
        self.verify_stage_bin(&ssh).inspect_err(|err| self.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Upload, None, err))?;
        log::info!("Auto-add {}: setup {}", self.target.host.host, self.target.layout.config);
        ssh.exec(
            &RemoteCommand::new(format!(
                "cd {} && {} setup --with-default-config --master-addr {}{}",
                shell_quote(&stage_root(&self.target.layout.stage_bin)),
                shell_quote(&self.target.layout.stage_bin),
                shell_quote(&ctx.master_addr_for(&self.target.host.host)?),
                self.target.layout.dir.as_deref().map(|dir| format!(" --directory {}", shell_quote(dir))).unwrap_or_default()
            ))
            .elevate(elevate),
        )
        .inspect_err(|err| self.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Setup, None, err))?;
        let setup = Self {
            minion_id: self
                .read_minion_id(&ssh)
                .inspect_err(|err| self.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Setup, None, err))?,
            ..self.clone()
        };
        setup
            .prepare_runtime(&ssh, elevate)
            .inspect_err(|err| setup.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Setup, setup.minion_id.as_deref(), err))?;
        log::info!("Auto-add {}: write onboarding traits {}", self.target.host.host, setup.target.layout.onboarding_traits);
        setup
            .write_onboarding_traits(&ssh, elevate)
            .inspect_err(|err| setup.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Setup, setup.minion_id.as_deref(), err))?;
        log::info!("Auto-add {}: register", self.target.host.host);
        setup
            .register(ctx, &ssh, elevate)
            .inspect_err(|err| setup.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Register, setup.minion_id.as_deref(), err))?;
        log::info!("Auto-add {}: start daemon", self.target.host.host);
        setup
            .start_runtime(&ssh, elevate)
            .inspect_err(|err| setup.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Start, setup.minion_id.as_deref(), err))?;
        log::info!("Auto-add {}: wait for daemon pid", self.target.host.host);
        setup
            .wait_runtime(&ssh, elevate)
            .inspect_err(|err| setup.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Start, setup.minion_id.as_deref(), err))?;
        log::info!("Auto-add {}: wait for bootstrap log", self.target.host.host);
        setup
            .wait_attempt(&ssh, elevate)
            .inspect_err(|err| setup.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Start, setup.minion_id.as_deref(), err))?;
        log::info!("Auto-add {}: wait for master readiness", self.target.host.host);
        setup
            .wait_ready(ctx, &ssh, elevate)
            .inspect_err(|err| setup.recover_add_failure(ctx, &ssh, elevate, AddFailureStage::Ready, setup.minion_id.as_deref(), err))?;
        log::info!("Auto-add {}: sync master CMDB", self.target.host.host);
        setup.sync_cmdb(ctx)?;
        Ok(())
    }

    fn sync_cmdb(&self, ctx: &SetupContext) -> Result<(), SysinspectError> {
        let Some(minion_id) = self.minion_id.as_deref() else {
            return Err(SysinspectError::MinionGeneralError(format!("Unable to sync CMDB for {}: minion id is missing", self.target.host.host)));
        };

        let context = json!({
            "user": self.target.host.user,
            "host": self.target.host.host,
            "root": self.target.layout.root_dir,
            "bin": self.target.layout.install_bin,
            "path": self.target.layout.config,
            "backend": "hopstart",
        })
        .to_string();

        let rsp = call_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_CMDB_UPSERT}"), "", Some(minion_id), Some(&context))?;
        if !matches!(rsp.payload, ConsolePayload::Ack { .. }) {
            return Err(SysinspectError::MasterGeneralError(format!("Master did not acknowledge CMDB update for {}", minion_id)));
        }

        Ok(())
    }

    fn recover_add_failure(
        &self, ctx: &SetupContext, ssh: &SSHSession, elevate: ElevationMode, stage: AddFailureStage, mid: Option<&str>, err: &SysinspectError,
    ) {
        log::warn!("Auto-add {}: cleanup after {:?} failure", self.target.host.host, stage);
        if matches!(stage, AddFailureStage::Register | AddFailureStage::Start | AddFailureStage::Ready)
            && let Some(mid) = mid
            && let Err(clean_err) = self.unregister_failed(ctx, mid)
        {
            log::warn!("Auto-add {}: cleanup could not unregister {}: {}", self.target.host.host, mid, clean_err);
        }
        if matches!(stage, AddFailureStage::Setup | AddFailureStage::Register | AddFailureStage::Start | AddFailureStage::Ready)
            && let Err(clean_err) = self.prepare_runtime(ssh, elevate)
        {
            log::warn!("Auto-add {}: cleanup could not stop partial runtime: {}", self.target.host.host, clean_err);
        }
        if let Err(clean_err) = self.cleanup_stage_root(ssh, elevate) {
            log::warn!("Auto-add {}: cleanup could not remove staged upload: {}", self.target.host.host, clean_err);
        }
        log::error!("Auto-add {}: {}", self.target.host.host, actionable_add_error(err));
    }

    fn start_runtime(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        ssh.exec(
            &RemoteCommand::new(format!("{} -c {} --daemon", shell_quote(&self.target.layout.install_bin), shell_quote(&self.target.layout.config)))
                .elevate(elevate),
        )
        .map(|_| ())
    }

    fn register(&self, ctx: &SetupContext, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let cmd = RemoteCommand::new(format!(
            "{} -c {} --register {}",
            shell_quote(&self.target.layout.install_bin),
            shell_quote(&self.target.layout.config),
            shell_quote(&ctx.master_fp)
        ))
        .elevate(elevate);
        match ssh.exec(&cmd) {
            Ok(rsp) => {
                if rsp.stdout.contains("Registration key mismatch for ")
                    && let Some(mid) = registration_mismatch_id(&rsp.stdout)
                {
                    log::warn!("Auto-add {}: removing stale master registration for {}", self.target.host.host, mid);
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
                log::warn!("Auto-add {}: removing stale master registration for {}", self.target.host.host, mid);
                self.unregister_stale(ctx, &mid)?;
                ssh.exec(&cmd).map(|_| ())
            }
        }
    }

    fn prepare_runtime(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let stop = format!(
            "{} -c {} --stop >/dev/null 2>&1 || true; rm -f {} {} {}",
            shell_quote(&self.target.layout.install_bin),
            shell_quote(&self.target.layout.config),
            shell_quote(&self.target.layout.pidfile),
            shell_quote(&self.target.layout.log_std),
            shell_quote(&self.target.layout.log_err)
        );
        ssh.exec(&RemoteCommand::new(stop).elevate(elevate)).map(|_| ())
    }

    fn write_onboarding_traits(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let yaml =
            self.onboarding_traits_map().into_iter().map(|(key, value)| format!("{key}: '{}'\n", value.replace('\'', "''"))).collect::<String>();
        let cmd = format!(
            "mkdir -p {0} && printf '%s' {1} > {2}",
            shell_quote(&self.target.layout.traits_dir),
            shell_quote(&yaml),
            shell_quote(&self.target.layout.onboarding_traits)
        );
        ssh.exec(&RemoteCommand::new(cmd).elevate(elevate)).map(|_| ())
    }

    fn onboarding_traits_map(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("minion.exec".to_string(), self.target.info.exec_mode.label().to_string()),
            ("minion.mode".to_string(), "daemon".to_string()),
            ("minion.sudo".to_string(), if self.target.info.has_sudo { "yes" } else { "no" }.to_string()),
            ("minion.version".to_string(), self.art.version.clone()),
        ])
    }

    fn read_minion_id(&self, ssh: &SSHSession) -> Result<Option<String>, SysinspectError> {
        let rsp = ssh.exec(&RemoteCommand::new(format!("cat {} 2>/dev/null || true", shell_quote(&self.target.layout.machine_id))))?;
        let mid = rsp.stdout.trim();
        Ok((!mid.is_empty()).then(|| mid.to_string()))
    }

    fn verify_stage_bin(&self, ssh: &SSHSession) -> Result<(), SysinspectError> {
        let chk = format!(
            "test -s {} && {} --version >/dev/null 2>&1",
            shell_quote(&self.target.layout.stage_bin),
            shell_quote(&self.target.layout.stage_bin)
        );
        match ssh.exec(&RemoteCommand::new(chk)) {
            Ok(_) => Ok(()),
            Err(_) => Err(SysinspectError::MinionGeneralError(format!(
                "Uploaded sysminion is not runnable on {}; {}",
                self.target.host.host,
                self.stage_snapshot(ssh)?
            ))),
        }
    }

    fn wait_runtime(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let deadline = Instant::now() + Duration::from_secs(12);
        let cmd = format!("test -s {0} && pid=$(cat {0}) && kill -0 \"$pid\"", shell_quote(&self.target.layout.pidfile));
        while Instant::now() < deadline {
            if ssh.exec(&RemoteCommand::new(cmd.clone()).elevate(elevate)).is_ok() {
                return Ok(());
            }
            sleep(Duration::from_millis(500));
        }
        Err(SysinspectError::MinionGeneralError(format!(
            "sysminion daemon did not stay alive on {}; {}",
            self.target.host.host,
            self.log_snapshot(ssh, elevate)?
        )))
    }

    fn wait_attempt(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            let rsp = ssh.exec(
                &RemoteCommand::new(format!(
                    "state=no; for p in {}; do if [ -f \"$p\" ] && grep -Eq {} \"$p\"; then state=fail; break; fi; if [ -f \"$p\" ] && grep -Eq {} \"$p\"; then state=yes; break; fi; done; printf '%s' \"$state\"",
                    self.log_candidates_expr(),
                    shell_quote("Minion encountered an error:|Unable to bootstrap secure transport|failed to lookup address information"),
                    shell_quote("Registration request to|Ehlo on|Secure session established|Unable to bootstrap secure transport")
                ))
                .elevate(elevate),
            );
            if let Ok(rsp) = rsp {
                match rsp.stdout.trim() {
                    "yes" => return Ok(()),
                    "fail" => {
                        return Err(SysinspectError::MinionGeneralError(format!(
                            "sysminion failed during bootstrap on {}; {}",
                            self.target.host.host,
                            self.log_snapshot(ssh, elevate)?
                        )));
                    }
                    _ => {}
                }
            }
            sleep(Duration::from_millis(500));
        }
        Err(SysinspectError::MinionGeneralError(format!(
            "sysminion did not log bootstrap progress in time on {}; {}",
            self.target.host.host,
            self.log_snapshot(ssh, elevate)?
        )))
    }

    fn wait_ready(&self, ctx: &SetupContext, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        let deadline = Instant::now() + Duration::from_secs(40);
        while Instant::now() < deadline {
            if self.readiness(ctx, ssh, elevate)?.ready() {
                return Ok(());
            }
            sleep(Duration::from_secs(1));
        }
        Err(SysinspectError::MinionGeneralError(format!(
            "sysminion started on {} but did not reach full master readiness in time; {} ({})",
            self.target.host.host,
            self.readiness(ctx, ssh, elevate)?.detail(),
            self.log_snapshot(ssh, elevate)?
        )))
    }

    fn readiness(&self, ctx: &SetupContext, ssh: &SSHSession, elevate: ElevationMode) -> Result<Readiness, SysinspectError> {
        Ok(Readiness {
            online: self.online(ctx)?,
            traits: self.has_traits(ctx)?,
            transport: self.has_transport(ctx)?,
            startup_sync: self.startup_sync_ready(ssh, elevate)?,
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
        let rsp = match call_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_MINION_INFO}"), "*", self.minion_id.as_deref(), None) {
            Ok(rsp) => rsp,
            Err(err) if is_waitable_console_miss(&err) => return Ok(false),
            Err(err) => return Err(err),
        };
        let ConsolePayload::MinionInfo { rows } = rsp.payload else {
            return Err(SysinspectError::MasterGeneralError("Master returned an unexpected payload while checking minion traits".to_string()));
        };
        Ok(rows_have_traits(&rows))
    }

    fn has_transport(&self, ctx: &SetupContext) -> Result<bool, SysinspectError> {
        let rsp = match call_console(
            &ctx.cfg,
            &format!("{SCHEME_COMMAND}{CLUSTER_TRANSPORT_STATUS}"),
            "*",
            self.minion_id.as_deref(),
            Some(&json!({ "filter": "all" }).to_string()),
        ) {
            Ok(rsp) => rsp,
            Err(err) if is_waitable_console_miss(&err) => return Ok(false),
            Err(err) => return Err(err),
        };
        let ConsolePayload::TransportStatus { rows } = rsp.payload else {
            return Err(SysinspectError::MasterGeneralError("Master returned an unexpected payload while checking transport state".to_string()));
        };
        Ok(rows.into_iter().any(|row| {
            self.minion_id.as_deref().is_some_and(|mid| row.minion_id == mid) && row.active_key_id.is_some() && row.last_handshake_at.is_some()
        }))
    }

    fn sensors_ready(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<bool, SysinspectError> {
        ssh.exec(
            &RemoteCommand::new(format!(
                "state=no; for p in {}; do if [ -f \"$p\" ] && grep -Eq {} \"$p\"; then state=yes; break; fi; done; printf '%s' \"$state\"",
                self.log_candidates_expr(),
                shell_quote("Sending sensors sync callback for cycle|Received sensors sync response from master")
            ))
            .elevate(elevate),
        )
        .map(|rsp| rsp.stdout.trim() == "yes")
    }

    fn startup_sync_ready(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<bool, SysinspectError> {
        ssh.exec(
            &RemoteCommand::new(format!(
                "state=no; for p in {}; do if [ -f \"$p\" ] && grep -Eq {} \"$p\"; then state=yes; break; fi; done; printf '%s' \"$state\"",
                self.log_candidates_expr(),
                shell_quote("Syncing modules from .* done|Module auto-sync .* is disabled")
            ))
            .elevate(elevate),
        )
        .map(|rsp| rsp.stdout.trim() == "yes")
    }

    fn matches_online_row(&self, row: &ConsoleOnlineMinionRow) -> bool {
        if self.minion_id.as_deref().is_some_and(|mid| row.minion_id == mid) {
            return true;
        }
        let host = self.target.host.host_norm.as_str();
        [row.fqdn.as_str(), row.hostname.as_str(), row.ip.as_str()]
            .into_iter()
            .map(normalise_remote_name)
            .any(|name| !name.is_empty() && name == host)
    }

    fn log_snapshot(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<String, SysinspectError> {
        let rsp = ssh.exec(
            &RemoteCommand::new(format!(
                "for p in {}; do if [ -f \"$p\" ]; then printf '%s: ' \"$p\"; tail -n 20 \"$p\" 2>/dev/null | tr '\\n' ' '; fi; done",
                self.log_candidates_expr()
            ))
            .elevate(elevate),
        )?;
        let out = rsp.stdout.trim();
        Ok(if out.is_empty() { "no remote logs found".to_string() } else { format!("remote logs: {out}") })
    }

    fn stage_snapshot(&self, ssh: &SSHSession) -> Result<String, SysinspectError> {
        let rsp = ssh.exec(&RemoteCommand::new(format!(
            "p={0}; d=$(dirname \"$p\"); ls -ld \"$d\" \"$p\" 2>/dev/null || true",
            shell_quote(&self.target.layout.stage_bin)
        )))?;
        let out = rsp.stdout.trim();
        Ok(if out.is_empty() { "uploaded file not visible on remote host".to_string() } else { out.to_string() })
    }

    fn log_candidates_expr(&self) -> String {
        format!(
            "{} {} {}/{} {}/{} /tmp/{} /tmp/{}",
            shell_quote(&self.target.layout.log_std),
            shell_quote(&self.target.layout.log_err),
            libsysinspect::cfg::mmconf::DEFAULT_MINION_SYSTEM_LOG_DIR,
            DEFAULT_MINION_LOG_STD,
            libsysinspect::cfg::mmconf::DEFAULT_MINION_SYSTEM_LOG_DIR,
            DEFAULT_MINION_LOG_ERR,
            DEFAULT_MINION_LOG_STD,
            DEFAULT_MINION_LOG_ERR
        )
    }

    fn unregister_stale(&self, ctx: &SetupContext, mid: &str) -> Result<(), SysinspectError> {
        let rsp = call_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_REMOVE_MINION}"), "", Some(mid), None)?;
        match rsp.payload {
            ConsolePayload::Ack { .. } => Ok(()),
            payload => Err(SysinspectError::MasterGeneralError(format!("Unable to remove stale minion {mid} before retry: {payload:?}"))),
        }
    }

    fn unregister_failed(&self, ctx: &SetupContext, mid: &str) -> Result<(), SysinspectError> {
        match call_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_REMOVE_MINION}"), "", Some(mid), None) {
            Ok(rsp) if matches!(rsp.payload, ConsolePayload::Ack { .. }) => Ok(()),
            Ok(_) => Ok(()),
            Err(err) if is_missing_master_minion(&err) => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn cleanup_stage_root(&self, ssh: &SSHSession, elevate: ElevationMode) -> Result<(), SysinspectError> {
        ssh.exec(&RemoteCommand::new(format!("rm -rf {}", shell_quote(&stage_root(&self.target.layout.stage_bin)))).elevate(elevate)).map(|_| ())
    }
}

impl NetworkAddWorkflow {
    fn upgrade_host(&self, ctx: &SetupContext, target: &ProbedHost) -> Result<AddStatus, SysinspectError> {
        let ssh = target.ssh();
        let elevate = target.elevation()?;
        if ssh
            .exec(
                &RemoteCommand::new(format!(
                    "state=no; for p in {} {} {}; do [ -e \"$p\" ] && state=yes && break; done; printf '%s' \"$state\"",
                    shell_quote(&target.layout.install_bin),
                    shell_quote(&target.layout.config),
                    shell_quote(&target.layout.root_dir)
                ))
                .elevate(elevate),
            )?
            .stdout
            .trim()
            != "yes"
        {
            return Ok(AddStatus::Absent);
        }
        if managed_roots(
            &ssh.exec(&RemoteCommand::new(format!("cat {} 2>/dev/null || true", shell_quote(&target.layout.local_marker))).elevate(elevate))?.stdout,
        )?
        .is_empty()
        {
            return Ok(AddStatus::Skipped);
        }
        let artifact = self.select_artifact(&ctx.repo_root, &target.info)?;
        if compare_versions(
            ssh.exec(&RemoteCommand::new(format!("{} --version 2>/dev/null || true", shell_quote(&target.layout.install_bin))))?
                .stdout
                .split_whitespace()
                .last()
                .unwrap_or_default(),
            &artifact.version,
        ) != std::cmp::Ordering::Less
        {
            return Ok(AddStatus::Current);
        }
        let setup = HostSetup {
            minion_id: ssh
                .exec(&RemoteCommand::new(format!("cat {} 2>/dev/null || true", shell_quote(&target.layout.machine_id))))?
                .stdout
                .trim()
                .to_string()
                .into(),
            target: target.clone(),
            art: artifact,
        };
        log::info!("Auto-upgrade {}: upload {}", target.host.host, setup.art.path.display());
        ssh.upload(&UploadRequest::new(&setup.art.path, &setup.target.layout.stage_bin).methods(vec![UploadMethod::Stream, UploadMethod::Scp]))?;
        ssh.exec(&RemoteCommand::new(format!("chmod 0755 {}", shell_quote(&setup.target.layout.stage_bin))))?;
        setup.verify_stage_bin(&ssh)?;
        setup.prepare_runtime(&ssh, elevate)?;
        log::info!("Auto-upgrade {}: refresh onboarding traits {}", target.host.host, setup.target.layout.onboarding_traits);
        setup.write_onboarding_traits(&ssh, elevate)?;
        log::info!("Auto-upgrade {}: replace {}", target.host.host, setup.target.layout.install_bin);
        ssh.exec(
            &RemoteCommand::new(format!(
                "cp {} {} && chmod 0755 {}",
                shell_quote(&setup.target.layout.stage_bin),
                shell_quote(&setup.target.layout.install_bin),
                shell_quote(&setup.target.layout.install_bin)
            ))
            .elevate(elevate),
        )?;
        log::info!("Auto-upgrade {}: start daemon", target.host.host);
        setup.start_runtime(&ssh, elevate)?;
        log::info!("Auto-upgrade {}: wait for daemon pid", target.host.host);
        setup.wait_runtime(&ssh, elevate)?;
        log::info!("Auto-upgrade {}: wait for bootstrap log", target.host.host);
        setup.wait_attempt(&ssh, elevate)?;
        log::info!("Auto-upgrade {}: wait for master readiness", target.host.host);
        setup.wait_ready(ctx, &ssh, elevate)?;
        log::info!("Auto-upgrade {}: sync master CMDB", target.host.host);
        setup.sync_cmdb(ctx)?;
        if let Some(mid) = setup.minion_id.as_deref() {
            let _ = call_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_SYNC}"), "*", Some(mid), None);
        }
        Ok(AddStatus::Upgraded)
    }

    fn remove_host(&self, ctx: &SetupContext, target: &ProbedHost) -> Result<AddStatus, SysinspectError> {
        let ssh = target.ssh();
        let elevate = target.elevation()?;
        let install_present = ssh
            .exec(
                &RemoteCommand::new(format!(
                    "state=no; for p in {} {} {} {}; do [ -e \"$p\" ] && state=yes && break; done; printf '%s' \"$state\"",
                    shell_quote(&target.layout.root_dir),
                    shell_quote(&target.layout.install_bin),
                    shell_quote(&target.layout.config),
                    shell_quote(&target.layout.local_marker)
                ))
                .elevate(elevate),
            )?
            .stdout
            .trim()
            == "yes";
        let marker =
            ssh.exec(&RemoteCommand::new(format!("cat {} 2>/dev/null || true", shell_quote(&target.layout.local_marker))).elevate(elevate))?.stdout;
        let roots = managed_roots(&marker)?;
        let minion_id =
            ssh.exec(&RemoteCommand::new(format!("cat {} 2>/dev/null || true", shell_quote(&target.layout.machine_id))))?.stdout.trim().to_string();
        let forced_master_cleanup = if minion_id.is_empty() && self.req.force { Some(self.force_remove_host(ctx, &target.host)?) } else { None };
        if !minion_id.is_empty() {
            match call_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_REMOVE_MINION}"), "", Some(&minion_id), None) {
                Ok(rsp) if matches!(rsp.payload, ConsolePayload::Ack { .. }) => {}
                Ok(rsp) => {
                    return Err(SysinspectError::MasterGeneralError(format!(
                        "Unable to unregister minion {} before removal: {:?}",
                        minion_id, rsp.payload
                    )));
                }
                Err(err) if is_missing_master_minion(&err) => {
                    log::warn!("Auto-remove {}: master no longer knows minion {}; continuing local cleanup", target.host.host, minion_id);
                }
                Err(err) => return Err(err),
            }
        }
        ssh.exec(
            &RemoteCommand::new(format!(
                "{} -c {} --stop >/dev/null 2>&1 || true",
                shell_quote(&target.layout.install_bin),
                shell_quote(&target.layout.config)
            ))
            .elevate(elevate),
        )?;
        if roots.is_empty() {
            ssh.exec(
                &RemoteCommand::new(format!(
                    "rm -rf {}",
                    shell_quote(
                        &std::path::Path::new(&target.layout.stage_bin)
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new(&target.layout.stage_bin))
                            .display()
                            .to_string()
                    )
                ))
                .elevate(elevate),
            )?;
            return Ok(forced_master_cleanup.unwrap_or(if install_present { AddStatus::Removed } else { AddStatus::Absent }));
        }
        ssh.exec(
            &RemoteCommand::new(format!(
                "rm -rf {} {}",
                roots.iter().map(|root| shell_quote(root)).collect::<Vec<_>>().join(" "),
                shell_quote(
                    &std::path::Path::new(&target.layout.stage_bin)
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new(&target.layout.stage_bin))
                        .display()
                        .to_string()
                )
            ))
            .elevate(elevate),
        )?;
        Ok(forced_master_cleanup.unwrap_or(AddStatus::Removed))
    }

    fn force_remove_host(&self, ctx: &SetupContext, host: &crate::netadd::types::AddHost) -> Result<AddStatus, SysinspectError> {
        match call_console(&ctx.cfg, &format!("{SCHEME_COMMAND}{CLUSTER_REMOVE_MINION}"), "", Some(&host.host), None) {
            Ok(rsp) if matches!(rsp.payload, ConsolePayload::Ack { .. }) => Ok(AddStatus::Removed),
            Ok(_) => Ok(AddStatus::Failed),
            Err(err) if is_missing_master_minion(&err) => Ok(AddStatus::Absent),
            Err(err) => Err(err),
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

pub(crate) fn is_waitable_console_miss(err: &SysinspectError) -> bool {
    let msg = err.to_string();
    msg.contains("requires one matching minion, but none were found") || msg.contains("requires exactly one matching minion, but 0 were selected")
}

pub(crate) fn is_missing_master_minion(err: &SysinspectError) -> bool {
    err.to_string().contains("Unable to find minion")
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
        self.online && self.traits && self.transport && self.startup_sync && self.sensors
    }

    fn detail(&self) -> String {
        format!(
            "{}: online={}, traits={}, transport={}, startup_sync={}, sensors={}",
            if self.online || self.traits || self.transport { "registered but not yet ready" } else { "not yet registered" },
            self.online,
            self.traits,
            self.transport,
            self.startup_sync,
            self.sensors
        )
    }
}

pub(crate) fn startup_sync_ready(log: &str) -> bool {
    (log.contains("Syncing modules from ") && log.contains(" done")) || log.contains("Module auto-sync on startup is disabled")
}

pub(crate) fn managed_roots(marker: &str) -> Result<Vec<String>, SysinspectError> {
    if marker.trim().is_empty() {
        return Ok(vec![]);
    }
    Ok(vec![LocalMarker::from_yaml(marker)?.root])
}

pub(crate) fn marker_matches_managed_root(expected_root: &str, marker: &str) -> bool {
    matches!(managed_roots(marker), Ok(roots) if roots.len() == 1 && roots[0] == expected_root)
}

pub(crate) fn classify_destination_state(expected_root: &str, destination_exists: bool, marker: &str, orphaned_traces: bool) -> ManagedInstallState {
    if destination_exists {
        return if marker_matches_managed_root(expected_root, marker) { ManagedInstallState::Managed } else { ManagedInstallState::NotManaged };
    }

    if orphaned_traces { ManagedInstallState::Broken } else { ManagedInstallState::Absent }
}

fn stage_root(stage_bin: &str) -> String {
    Path::new(stage_bin).parent().unwrap_or_else(|| Path::new(stage_bin)).display().to_string()
}

pub(crate) fn rows_have_traits(rows: &[ConsoleMinionInfoRow]) -> bool {
    rows.iter().any(|row| row.key == "minion.online" && row.value.as_bool() == Some(true))
        && rows.iter().any(|row| row.key == "system.id")
        && rows.iter().any(|row| row.key == "system.hostname")
}

pub(crate) fn actionable_add_error(err: &SysinspectError) -> String {
    if err.to_string().contains("Another minion from this machine is already connected") {
        return "master still sees a stale live session for this minion".to_string();
    }
    if err.to_string().contains("Registration key mismatch") {
        return "registration key mismatch stayed unresolved during auto-add".to_string();
    }
    if err.to_string().contains("did not reach full master readiness") && err.to_string().contains("transport=false") {
        return "minion registered traits, but secure transport never became ready".to_string();
    }
    err.to_string()
}
