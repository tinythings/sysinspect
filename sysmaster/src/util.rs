use colored::Colorize;
use libsysinspect::cfg::mmconf::MasterConfig;
use std::collections::HashSet;

/// Check sensor exports in config against physical sensors and return the valid ones.
/// If `logit` is true, log warnings for invalid exports and info for physical sensors not exported.
pub(crate) fn log_sensors_export(cfg: &MasterConfig, logit: bool) -> Vec<String> {
    let sroot = cfg.fileserver_sensors_root();
    let mut conf = cfg.fileserver_sensors();
    conf.sort();

    let mut phys: Vec<String> = std::fs::read_dir(&sroot)
        .ok()
        .into_iter()
        .flat_map(|rd| rd.filter_map(Result::ok))
        .filter_map(|e| {
            let ft = e.file_type().ok()?;
            if !ft.is_dir() {
                return None;
            }
            Some(e.file_name().to_string_lossy().to_string())
        })
        .collect();
    phys.sort();

    let pset = phys.iter().cloned().collect::<HashSet<String>>();
    let mut ok: Vec<String> = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for s in conf {
        if pset.contains(&s) {
            ok.push(s);
        } else {
            bad.push(s);
        }
    }

    if logit {
        if !bad.is_empty() {
            let ls = bad.iter().map(|s| format!("\"{s}\"").bright_red().to_string()).collect::<Vec<_>>().join(", ");
            log::warn!("Bogus sensor exports in fileserver.sensors: {ls}");
        }

        let oset = ok.iter().cloned().collect::<HashSet<String>>();
        let mut miss: Vec<String> = phys.into_iter().filter(|s| !oset.contains(s)).collect();
        miss.sort();
        if !miss.is_empty() {
            let ls = miss.iter().map(|s| format!("\"{s}\"").bright_cyan().to_string()).collect::<Vec<_>>().join(", ");
            log::info!("Physical sensors present but not exported in fileserver.sensors: {ls}");
        }

        if !bad.is_empty() && ok.is_empty() {
            log::warn!("fileserver.sensors has no useful entries: all configured sensor exports are bogus");
        }
    }

    ok
}
