mod callbacks;
mod clidef;
mod filedata;
mod minion;
mod proto;
mod ptcounter;
mod rsa;

#[cfg(test)]
mod filedata_ut;

#[cfg(test)]
mod minion_ut;

#[cfg(test)]
mod proto_ut;

#[cfg(test)]
mod rsa_ut;

#[cfg(test)]
mod registration_ut;

#[cfg(test)]
mod setup_ut;

#[cfg(test)]
mod start_ut;

use clap::{ArgMatches, Command};
use clidef::cli;
use daemonize::Daemonize;
use libcommon::SysinspectError;
use libsysinspect::{
    cfg::{get_minion_config, mmconf::MinionConfig},
    inspector::SysInspectRunner,
    logger,
};
use log::LevelFilter;
use std::{collections::BTreeSet, env, fs::File, process::exit, sync::Arc, sync::OnceLock};
use tokio::task::JoinHandle;

use crate::minion::SysMinion;

static APPNAME: &str = "sysminion";
static VERSION: &str = env!("CARGO_PKG_VERSION");
static LOGGER: OnceLock<logger::STDOUTLogger> = OnceLock::new();

fn runtime(worker_threads: usize, max_blocking_threads: usize) -> Result<tokio::runtime::Runtime, SysinspectError> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .max_blocking_threads(max_blocking_threads)
        .enable_all()
        .build()
        .map_err(|e| SysinspectError::DynError(Box::new(e)))
}

fn register_minion(cfg: MinionConfig, fp: String) -> Result<(), SysinspectError> {
    let runtime = runtime(cfg.performance().register_threads().0, cfg.performance().register_threads().1)?;
    let dpq = Arc::new(libdpq::DiskPersistentQueue::open(cfg.pending_tasks_dir())?);
    SysInspectRunner::set_dpq(dpq.clone());
    runtime.block_on(async { minion::_minion_instance(cfg, Some(fp), dpq).await })?;
    Ok(())
}

fn start_minion(cfg: MinionConfig, fp: Option<String>) -> Result<(), SysinspectError> {
    let runtime = runtime(cfg.performance().daemon_threads().0, cfg.performance().daemon_threads().1)?;
    runtime.block_on(async {
        loop {
            let c_cfg = cfg.clone();
            let c_fp = fp.clone();
            let h: JoinHandle<()> = tokio::spawn(async move {
                minion::minion(c_cfg, c_fp).await;
            });

            log::info!("Minion process started");

            match h.await {
                Ok(_) => println!("Minion task completed."),
                Err(e) if e.is_cancelled() => println!("Minion task was aborted."),
                Err(e) => println!("Minion task failed: {e:?}"),
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    Ok(())
}

fn running_minion_targets(cfg: &MinionConfig, sniffed: &[i32], current: i32) -> Vec<i32> {
    std::fs::read_to_string(cfg.pidfile())
        .ok()
        .and_then(|raw| raw.trim().parse::<i32>().ok())
        .filter(|pid| sniffed.contains(pid))
        .into_iter()
        .chain(sniffed.iter().copied())
        .filter(|pid| *pid != current)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn sniff_minion_pids_sync() -> Result<Vec<i32>, SysinspectError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| SysinspectError::DynError(Box::new(e)))?
        .block_on(sniff_minion_pids())
        .map_err(SysinspectError::IoErr)
}

fn get_config(params: &ArgMatches) -> MinionConfig {
    // Config
    match get_minion_config(Some(params.get_one::<String>("config").map_or("", |v| v))) {
        Ok(cfg) => cfg,
        Err(err) => {
            log::error!("Unable to find a Minion config: {err}");
            exit(1);
        }
    }
}

fn stop_targets(cfg: &MinionConfig, sniffed: &[i32], current: i32) -> Vec<i32> {
    running_minion_targets(cfg, sniffed, current)
}

async fn sniff_minion_pids() -> std::io::Result<Vec<i32>> {
    use procdog::ProcBackend;

    #[cfg(target_os = "linux")]
    return Ok(procdog::backends::linuxps::LinuxPsBackend
        .list()
        .await?
        .into_iter()
        .filter_map(|(pid, name)| (name == APPNAME).then_some(pid))
        .collect());

    #[cfg(target_os = "netbsd")]
    return Ok(procdog::backends::netbsd_sysctl::NetBsdSysctlBackend
        .list()
        .await?
        .into_iter()
        .filter_map(|(pid, name)| (name == APPNAME).then_some(pid))
        .collect());

    #[cfg(all(not(target_os = "linux"), not(target_os = "netbsd")))]
    Ok(procdog::backends::stps::PsBackend.list().await?.into_iter().filter_map(|(pid, name)| (name == APPNAME).then_some(pid)).collect())
}

fn stop_minion(cfg: MinionConfig) -> Result<(), SysinspectError> {
    for pid in stop_targets(&cfg, &sniff_minion_pids_sync()?, std::process::id() as i32) {
        libsysinspect::util::sys::kill_pid(pid, Some(2)).map_err(SysinspectError::IoErr)?;
    }
    Ok(())
}

fn start_minion_daemon(cfg: MinionConfig, fp: Option<String>) -> Result<(), SysinspectError> {
    if !running_minion_targets(&cfg, &sniff_minion_pids_sync()?, std::process::id() as i32).is_empty() {
        log::info!("Minion already running, daemon wake request is idempotent");
        return Ok(());
    }

    let sout = File::create(cfg.logfile_std()).map_err(|err| {
        SysinspectError::ConfigError(format!("Unable to create main log file at {}: {err}", cfg.logfile_std().to_str().unwrap_or_default()))
    })?;
    log::info!("Opened main log file at {}", cfg.logfile_std().to_str().unwrap_or_default());

    let serr = File::create(cfg.logfile_err())
        .map_err(|err| SysinspectError::ConfigError(format!("Unable to create file at {}: {err}", cfg.logfile_err().to_str().unwrap_or_default())))?;
    log::info!("Opened error log file at {}", cfg.logfile_err().to_str().unwrap_or_default());

    match Daemonize::new().pid_file(cfg.pidfile()).stdout(sout).stderr(serr).start() {
        Ok(_) => {
            log::info!("Daemon started with PID file at {}", cfg.pidfile().to_str().unwrap_or_default());
            start_minion(cfg, fp)
        }
        Err(err) => Err(SysinspectError::ConfigError(format!("Error starting minion in daemon mode: {err}"))),
    }
}

// Print help?
fn help(cli: &mut Command, params: ArgMatches) -> bool {
    for sc in ["setup", "module"] {
        if let Some(sub) = params.subcommand_matches(sc)
            && sub.get_flag("help")
        {
            if let Some(s_cli) = cli.find_subcommand_mut(sc) {
                _ = s_cli.print_help();
                return true;
            }
            return false;
        }
    }

    if params.get_flag("help") {
        _ = &cli.print_long_help();
        return true;
    }

    // Print a global version?
    if params.get_flag("version") {
        println!("Version: {APPNAME} {VERSION}");
        return true;
    }

    false
}

fn main() -> std::io::Result<()> {
    let mut cli = cli(VERSION, APPNAME);
    if env::args().collect::<Vec<String>>().len() == 1 {
        cli.print_help()?;
        std::process::exit(1);
    }

    let params = cli.to_owned().get_matches();

    // Print helps, versions etc
    if help(&mut cli, params.clone()) {
        std::process::exit(0);
    }

    // Setup logger
    if let Err(err) = log::set_logger(LOGGER.get_or_init(|| logger::STDOUTLogger::new(params.get_flag("no-color")))).map(|()| {
        log::set_max_level(match params.get_count("debug") {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            2.. => LevelFilter::max(),
        })
    }) {
        println!("Error setting logger output: {err}");
    }

    // Start
    let fp = params.get_one::<String>("register").cloned();
    if let Some(fp) = fp {
        let cfg = get_config(&params);
        if let Err(err) = register_minion(cfg, fp) {
            log::error!("Error registering minion: {err}");
            exit(1);
        }
    } else if params.get_flag("start") {
        let cfg = get_config(&params);
        if let Err(err) = start_minion(cfg, None) {
            log::error!("Error starting minion: {err}");
            exit(1);
        }
    } else if params.get_flag("daemon") {
        log::info!("Starting daemon");
        let cfg = get_config(&params);
        if let Err(err) = start_minion_daemon(cfg, fp) {
            log::error!("Error starting minion in daemon mode: {err}");
            exit(1)
        }
    } else if params.get_flag("stop") {
        log::info!("Stopping minion");
        let cfg = get_config(&params);
        if let Err(err) = stop_minion(cfg) {
            log::error!("Unable to stop sysminion: {err}");
        }
    } else if let Some(sub) = params.subcommand_matches("setup") {
        if let Err(err) = minion::setup(sub) {
            log::error!("Error running setup: {err}");
            exit(1);
        }
    } else if let Some(sub) = params.subcommand_matches("module") {
        if let Err(err) = minion::launch_module(get_config(&params), sub) {
            log::error!("Error launching module: {err}");
            exit(1);
        }
    } else if params.get_flag("info") {
        SysMinion::print_info(&get_config(&params));
    } else {
        cli.print_help()?;
    }

    Ok(())
}

#[cfg(test)]
mod stop_ut {
    use super::{running_minion_targets, stop_targets};
    use libsysinspect::cfg::mmconf::MinionConfig;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn scratch_pidfile() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sysminion-stop-ut-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir.join("sysminion.pid")
    }

    #[test]
    fn stop_targets_merge_pidfile_and_sniffed_without_self() {
        let pidfile = scratch_pidfile();
        fs::write(&pidfile, "42\n").unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_pid_path(pidfile.to_str().unwrap());

        assert_eq!(stop_targets(&cfg, &[42, 43, 77], 77), vec![42, 43]);

        let _ = fs::remove_file(pidfile);
    }

    #[test]
    fn stale_pidfile_does_not_fake_running_minion() {
        let pidfile = scratch_pidfile();
        fs::write(&pidfile, "42\n").unwrap();
        let mut cfg = MinionConfig::default();
        cfg.set_pid_path(pidfile.to_str().unwrap());

        assert_eq!(running_minion_targets(&cfg, &[43, 77], 77), vec![43]);

        let _ = fs::remove_file(pidfile);
    }
}
