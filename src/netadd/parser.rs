use crate::netadd::types::{AddHost, AddKey, AddPlan, AddRequest, HostSpec, ResolvedDest};
use crate::netadd::workflow::NetworkAddWorkflow;
use clap::ArgMatches;
use colored::Colorize;
use libcommon::SysinspectError;
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};

/// Parse and resolve one `network --add` request.
pub(crate) fn parse(am: &ArgMatches) -> Result<AddPlan, SysinspectError> {
    NetworkAddWorkflow::from_matches(am)?.plan()
}

pub(crate) fn parse_request(am: &ArgMatches) -> Result<AddRequest, SysinspectError> {
    if !am.get_flag("add") {
        return Err(SysinspectError::InvalidQuery("Host onboarding requires --add".to_string()));
    }
    if am.get_one::<String>("query-pos").is_some_and(|v| v != "*") {
        return Err(SysinspectError::InvalidQuery("Invalid input: host onboarding does not accept positional selectors".to_string()));
    }

    let hosts = if let Some(v) = am.get_one::<String>("hostnames") {
        parse_hostnames(v)?
    } else if let Some(p) = am.get_one::<String>("list") {
        parse_list(Path::new(p))?
    } else {
        return Err(SysinspectError::InvalidQuery("Invalid input: specify either --hostnames or --list for --add".to_string()));
    };
    if hosts.is_empty() {
        return Err(SysinspectError::InvalidQuery("Invalid input: no host entries were supplied for --add".to_string()));
    }

    Ok(AddRequest {
        hosts,
        user: am
            .get_one::<String>("user")
            .cloned()
            .or_else(current_user)
            .ok_or_else(|| SysinspectError::ConfigError("Invalid input: no default SSH user could be resolved".to_string()))?,
    })
}

pub(crate) fn resolve_plan(req: &AddRequest) -> Result<AddPlan, SysinspectError> {
    let mut seen = BTreeSet::<AddKey>::new();
    let mut items = Vec::<AddHost>::new();

    for spec in &req.hosts {
        let item = AddHost {
            raw: spec.raw.clone(),
            host: spec.host.clone(),
            host_norm: normalise_host(&spec.host),
            user: spec.user.clone().unwrap_or_else(|| req.user.clone()),
            path: spec.path.clone(),
            path_norm: normalise_path(spec.path.as_deref()),
        };
        let key = AddKey { user: item.user.clone(), host: item.host_norm.clone(), path: item.path_norm.clone() };
        if !seen.insert(key) {
            return Err(SysinspectError::InvalidQuery(format!(
                "Invalid input: duplicate host entry after normalisation: {}",
                item.raw.bright_yellow()
            )));
        }
        items.push(item);
    }

    items.sort_by(|a, b| (&a.host_norm, &a.user, &a.path_norm).cmp(&(&b.host_norm, &b.user, &b.path_norm)));
    Ok(AddPlan { items })
}

pub(crate) fn parse_hostnames(value: &str) -> Result<Vec<HostSpec>, SysinspectError> {
    value.split(',').map(str::trim).filter(|v| !v.is_empty()).map(parse_entry).collect()
}

pub(crate) fn parse_list(path: &Path) -> Result<Vec<HostSpec>, SysinspectError> {
    fs::read_to_string(path)
        .map_err(|err| SysinspectError::InvalidQuery(format!("Invalid input: cannot read host list {}: {err}", path.display())))?
        .lines()
        .map(str::trim)
        .filter(|v| !v.is_empty() && !v.starts_with('#'))
        .map(parse_entry)
        .collect()
}

pub(crate) fn parse_entry(raw: &str) -> Result<HostSpec, SysinspectError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(SysinspectError::InvalidQuery("Invalid input: empty host entry is not allowed".to_string()));
    }

    let (user, rest) = match raw.split_once('@') {
        Some((user, rest)) if user.is_empty() || rest.is_empty() || rest.contains('@') => {
            return Err(SysinspectError::InvalidQuery(format!("Invalid input: invalid host entry: {raw}")));
        }
        Some((user, rest)) => (Some(user.to_string()), rest),
        None => (None, raw),
    };
    let (host, path) = match rest.split_once(':') {
        Some((host, path)) if host.is_empty() || path.is_empty() => {
            return Err(SysinspectError::InvalidQuery(format!("Invalid input: invalid host entry: {raw}")));
        }
        Some((host, _)) if rest[host.len() + 1..].contains(':') => {
            return Err(SysinspectError::InvalidQuery(format!("Invalid input: host entry contains too many ':' separators: {raw}")));
        }
        Some((host, path)) => (host.to_string(), Some(path.to_string())),
        None => (rest.to_string(), None),
    };
    if host.trim().is_empty() {
        return Err(SysinspectError::InvalidQuery(format!("Invalid input: invalid host entry: {raw}")));
    }

    Ok(HostSpec { raw: raw.to_string(), user, host, path })
}

pub(crate) fn current_user() -> Option<String> {
    ["USER", "LOGNAME", "USERNAME"].into_iter().find_map(|k| env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()))
}

pub(crate) fn normalise_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

pub(crate) fn normalise_path(path: Option<&str>) -> Option<String> {
    let path = path.map(str::trim).filter(|v| !v.is_empty())?;
    if path == "/" {
        return Some("/".to_string());
    }
    Some(path.trim_end_matches('/').to_string())
}

/// Resolve one relative remote path under a known remote home directory.
pub(crate) fn resolve_remote_path(home: &Path, path: Option<&str>) -> Option<PathBuf> {
    match normalise_path(path) {
        Some(path) if path.starts_with('/') => Some(PathBuf::from(path)),
        Some(path) => Some(home.join(path)),
        None => None,
    }
}

/// Resolve one destination root against the remote home when needed.
pub(crate) fn resolve_dest(home: &Path, path: Option<&str>) -> ResolvedDest {
    ResolvedDest { input: normalise_path(path), path: resolve_remote_path(home, path) }
}
