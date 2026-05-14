use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use std::collections::HashMap;
use std::path::Path;

use crate::db;

/// Main entry point. Dispatches to the appropriate operation.
pub fn run(rt: &ModRequest) -> ModResponse {
    let mut resp = runtime::new_call_response();
    let name = runtime::get_arg(rt, "name");

    if name.is_empty() {
        resp.set_retcode(1);
        resp.set_message("Argument \"name\" is required");
        return resp;
    }

    if runtime::get_opt(rt, "check") {
        return check_user(&name, &mut resp);
    }
    if runtime::get_opt(rt, "present") {
        return ensure_present(rt, &name, &mut resp);
    }
    if runtime::get_opt(rt, "absent") {
        return ensure_absent(rt, &name, &mut resp);
    }
    if runtime::get_opt(rt, "group-present") {
        return ensure_group_present(rt, &name, &mut resp);
    }
    if runtime::get_opt(rt, "group-absent") {
        return ensure_group_absent(rt, &name, &mut resp);
    }

    resp.set_retcode(1);
    resp.set_message("No operation specified. Use --check, --present, --absent, --group-present, or --group-absent");
    resp
}

fn check_user(name: &str, resp: &mut ModResponse) -> ModResponse {
    let mut data = HashMap::new();
    data.insert("name".to_string(), serde_json::Value::String(name.to_string()));

    let passwd_lines = db::read_db(db::passwd_path());
    let entry = passwd_lines.iter().find_map(|l| db::parse_passwd(l).filter(|e| e.name == name));

    match entry {
        Some(u) => {
            let groups = get_user_groups(name);
            data.insert("exists".to_string(), serde_json::Value::Bool(true));
            data.insert("uid".to_string(), serde_json::Value::Number(serde_json::Number::from(u.uid)));
            data.insert("gid".to_string(), serde_json::Value::Number(serde_json::Number::from(u.gid)));
            data.insert("home".to_string(), serde_json::Value::String(u.home));
            data.insert("shell".to_string(), serde_json::Value::String(u.shell));
            data.insert("groups".to_string(), serde_json::Value::Array(groups.iter().map(|g| serde_json::Value::String(g.clone())).collect()));
            resp.set_retcode(0);
            resp.set_message(&format!("User '{}' exists (uid={})", name, u.uid));
        }
        None => {
            data.insert("exists".to_string(), serde_json::Value::Bool(false));
            resp.set_retcode(0);
            resp.set_message(&format!("User '{}' does not exist", name));
        }
    }

    if let Err(e) = resp.set_data(&data) {
        resp.add_warning(&format!("{e}"));
    }
    resp.clone()
}

fn get_user_groups(name: &str) -> Vec<String> {
    let group_lines = db::read_db(db::group_path());
    group_lines.iter().filter_map(|l| db::parse_group(l)).filter(|g| g.members.contains(&name.to_string())).map(|g| g.name).collect()
}

fn ensure_present(rt: &ModRequest, name: &str, resp: &mut ModResponse) -> ModResponse {
    let dry_run = runtime::get_opt(rt, "dry-run");
    let uid_arg = runtime::get_arg(rt, "uid");
    let gid_arg = runtime::get_arg(rt, "gid");
    let home_arg = runtime::get_arg(rt, "home");
    let shell_arg = runtime::get_arg(rt, "shell");

    let mut passwd = db::read_db(db::passwd_path());
    let mut group_lines = db::read_db(db::group_path());
    let existing = passwd.iter().find_map(|l| db::parse_passwd(l).filter(|e| e.name == name));

    let uid: u32 = uid_arg.parse().unwrap_or_else(|_| db::find_free_uid(&passwd, 1000));
    let gid: u32 = gid_arg.parse().unwrap_or(uid);
    let home = if home_arg.is_empty() { format!("/home/{name}") } else { home_arg };
    let shell = if shell_arg.is_empty() { db::default_shell().to_string() } else { shell_arg };

    if let Some(ref e) = existing {
        let same = e.uid == uid && e.gid == gid && e.home == home && e.shell == shell;
        if same {
            resp.set_retcode(0);
            resp.set_message(&format!("User '{}' already exists with matching attributes", name));
            let data = user_telemetry(name, uid, gid, &home, &shell, false);
            if let Err(e) = resp.set_data(&data) {
                resp.add_warning(&format!("{e}"));
            }
            return resp.clone();
        }
    }

    if dry_run {
        resp.set_retcode(0);
        resp.set_message(&format!("[dry-run] would create/update user '{}' (uid={}, gid={})", name, uid, gid));
        return resp.clone();
    }

    let gecos = existing.as_ref().map_or(String::new(), |e| e.gecos.clone());
    let new_entry = db::PasswdEntry { name: name.to_string(), uid, gid, gecos, home: home.clone(), shell: shell.clone() };
    let new_line = db::format_passwd(&new_entry);

    if let Some(ref e) = existing {
        for line in passwd.iter_mut() {
            if db::parse_passwd(line).is_some_and(|p| p.name == e.name) {
                *line = new_line.clone();
                break;
            }
        }
    } else {
        passwd.push(new_line);
    }

    // Create primary group if it doesn't exist
    ensure_group(&mut group_lines, name, gid);

    if let Err(e) = db::write_db(db::passwd_path(), &passwd) {
        resp.set_retcode(1);
        resp.set_message(&format!("Failed to write passwd: {e}"));
        return resp.clone();
    }

    if let Err(e) = db::write_db(db::group_path(), &group_lines) {
        resp.set_retcode(1);
        resp.set_message(&format!("Failed to write group: {e}"));
        return resp.clone();
    }

    if !Path::new(&home).exists() {
        let _ = std::fs::create_dir_all(&home);
        let _ = chown_path(&home, uid, gid);
    }

    resp.set_retcode(0);
    resp.set_message(&format!("User '{}' {} (uid={}, gid={})", name, if existing.is_some() { "updated" } else { "created" }, uid, gid));
    let data = user_telemetry(name, uid, gid, &home, &shell, true);
    if let Err(e) = resp.set_data(&data) {
        resp.add_warning(&format!("{e}"));
    }
    resp.clone()
}

fn ensure_absent(rt: &ModRequest, name: &str, resp: &mut ModResponse) -> ModResponse {
    let dry_run = runtime::get_opt(rt, "dry-run");
    let remove_home = runtime::get_opt(rt, "remove-home");

    let mut passwd = db::read_db(db::passwd_path());
    let existing = passwd.iter().find_map(|l| db::parse_passwd(l).filter(|e| e.name == name));

    if existing.is_none() {
        resp.set_retcode(0);
        resp.set_message(&format!("User '{}' does not exist", name));
        let mut data = HashMap::new();
        data.insert("name".to_string(), serde_json::Value::String(name.to_string()));
        data.insert("changed".to_string(), serde_json::Value::Bool(false));
        if let Err(e) = resp.set_data(&data) {
            resp.add_warning(&format!("{e}"));
        }
        return resp.clone();
    }

    if dry_run {
        resp.set_retcode(0);
        resp.set_message(&format!("[dry-run] would remove user '{}'", name));
        return resp.clone();
    }

    let e = existing.unwrap();
    passwd.retain(|l| db::parse_passwd(l).is_none_or(|p| p.name != name));

    // Remove from all groups
    let mut groups = db::read_db(db::group_path());
    for line in groups.iter_mut() {
        if let Some(mut g) = db::parse_group(line)
            && g.members.contains(&name.to_string()) {
                g.members.retain(|m| m != name);
                *line = db::format_group(&g);
            }
    }

    if let Err(e) = db::write_db(db::passwd_path(), &passwd) {
        resp.set_retcode(1);
        resp.set_message(&format!("Failed to write passwd: {e}"));
        return resp.clone();
    }
    if let Err(e) = db::write_db(db::group_path(), &groups) {
        resp.set_retcode(1);
        resp.set_message(&format!("Failed to write group: {e}"));
        return resp.clone();
    }

    if remove_home && !e.home.is_empty() {
        let _ = std::fs::remove_dir_all(&e.home);
    }

    resp.set_retcode(0);
    resp.set_message(&format!("User '{}' removed", name));
    let data = user_telemetry(name, e.uid, e.gid, &e.home, &e.shell, true);
    if let Err(e) = resp.set_data(&data) {
        resp.add_warning(&format!("{e}"));
    }
    resp.clone()
}

fn ensure_group_present(rt: &ModRequest, name: &str, resp: &mut ModResponse) -> ModResponse {
    let gid_arg = runtime::get_arg(rt, "gid");
    let dry_run = runtime::get_opt(rt, "dry-run");

    let mut groups = db::read_db(db::group_path());
    let gid: u32 = gid_arg.parse().unwrap_or_else(|_| db::find_free_gid(&groups, 1000));

    ensure_group(&mut groups, name, gid);

    if dry_run {
        resp.set_retcode(0);
        resp.set_message(&format!("[dry-run] would ensure group '{}' (gid={})", name, gid));
        return resp.clone();
    }

    if let Err(e) = db::write_db(db::group_path(), &groups) {
        resp.set_retcode(1);
        resp.set_message(&format!("Failed to write group: {e}"));
        return resp.clone();
    }

    resp.set_retcode(0);
    resp.set_message(&format!("Group '{}' ensured (gid={})", name, gid));
    let mut data = HashMap::new();
    data.insert("name".to_string(), serde_json::Value::String(name.to_string()));
    data.insert("gid".to_string(), serde_json::Value::Number(serde_json::Number::from(gid)));
    data.insert("changed".to_string(), serde_json::Value::Bool(true));
    if let Err(e) = resp.set_data(&data) {
        resp.add_warning(&format!("{e}"));
    }
    resp.clone()
}

fn ensure_group_absent(rt: &ModRequest, name: &str, resp: &mut ModResponse) -> ModResponse {
    let dry_run = runtime::get_opt(rt, "dry-run");

    let mut groups = db::read_db(db::group_path());
    let existing = groups.iter().any(|l| db::parse_group(l).is_some_and(|g| g.name == name));

    if !existing {
        resp.set_retcode(0);
        resp.set_message(&format!("Group '{}' does not exist", name));
        return resp.clone();
    }

    if dry_run {
        resp.set_retcode(0);
        resp.set_message(&format!("[dry-run] would remove group '{}'", name));
        return resp.clone();
    }

    groups.retain(|l| db::parse_group(l).is_none_or(|g| g.name != name));

    if let Err(e) = db::write_db(db::group_path(), &groups) {
        resp.set_retcode(1);
        resp.set_message(&format!("Failed to write group: {e}"));
        return resp.clone();
    }

    resp.set_retcode(0);
    resp.set_message(&format!("Group '{}' removed", name));
    resp.clone()
}

fn ensure_group(groups: &mut Vec<String>, name: &str, gid: u32) {
    if !groups.iter().any(|l| db::parse_group(l).is_some_and(|g| g.name == name)) {
        let new_group = db::GroupEntry { name: name.to_string(), gid, members: Vec::new() };
        groups.push(db::format_group(&new_group));
    }
}

fn user_telemetry(name: &str, uid: u32, gid: u32, home: &str, shell: &str, changed: bool) -> HashMap<String, serde_json::Value> {
    let mut m = HashMap::new();
    m.insert("name".to_string(), serde_json::Value::String(name.to_string()));
    m.insert("uid".to_string(), serde_json::Value::Number(serde_json::Number::from(uid)));
    m.insert("gid".to_string(), serde_json::Value::Number(serde_json::Number::from(gid)));
    m.insert("home".to_string(), serde_json::Value::String(home.to_string()));
    m.insert("shell".to_string(), serde_json::Value::String(shell.to_string()));
    m.insert("changed".to_string(), serde_json::Value::Bool(changed));
    m
}

fn chown_path(path: &str, uid: u32, gid: u32) -> Result<(), ()> {
    let cpath = std::ffi::CString::new(path).map_err(|_| ())?;
    let ret = unsafe { libc::chown(cpath.as_ptr(), uid, gid) };
    if ret == 0 { Ok(()) } else { Err(()) }
}
