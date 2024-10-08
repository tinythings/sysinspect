use libsysinspect::modlib::{
    response::ModResponse,
    runtime::{self, ArgValue, ModRequest},
};
use procfs::process::{all_processes, LimitValue, Process};
use serde_json::json;
use std::{collections::HashMap, vec};

/// Return process, if found
fn find_process(cmd: String) -> Option<procfs::process::Process> {
    for p in all_processes().unwrap() {
        if let Ok(p) = p {
            if let Ok(cmdline) = p.cmdline() {
                if cmdline.join(" ").starts_with(&cmd) {
                    return Some(p);
                }
            }
        }
    }

    None
}

/// Get a string argument
fn get_arg(rt: &ModRequest, arg: &str) -> String {
    if let Some(s_arg) = rt.first_arg(arg) {
        if let Some(s_arg) = s_arg.as_string() {
            return s_arg;
        } else if let Some(s_arg) = s_arg.as_bool() {
            return format!("{}", s_arg);
        }
    }
    "".to_string()
}

/// Get a presence of a flag/option
fn get_opt(rt: &ModRequest, opt: &str) -> bool {
    for av in rt.options() {
        if av.as_string().unwrap_or_default().eq(opt) {
            return true;
        }
    }
    false
}

/// Get process limits
fn get_limits(p: Process) -> HashMap<String, Vec<serde_json::Value>> {
    fn s(l: LimitValue) -> serde_json::Value {
        match l {
            LimitValue::Unlimited => json!("unlimited"),
            LimitValue::Value(v) => json!(v),
        }
    }
    let l = p.limits().unwrap();
    let mut out: HashMap<String, Vec<serde_json::Value>> = HashMap::default();
    out.insert("cpu time".to_string(), vec![s(l.max_cpu_time.soft_limit), s(l.max_cpu_time.hard_limit)]);
    out.insert("file size".to_string(), vec![s(l.max_file_size.soft_limit), s(l.max_file_size.hard_limit)]);
    out.insert("data size".to_string(), vec![s(l.max_data_size.soft_limit), s(l.max_data_size.hard_limit)]);
    out.insert("stack size".to_string(), vec![s(l.max_stack_size.soft_limit), s(l.max_stack_size.hard_limit)]);
    out.insert("core file size".to_string(), vec![s(l.max_core_file_size.soft_limit), s(l.max_core_file_size.hard_limit)]);
    out.insert("resident set".to_string(), vec![s(l.max_resident_set.soft_limit), s(l.max_resident_set.hard_limit)]);
    out.insert("processes".to_string(), vec![s(l.max_processes.soft_limit), s(l.max_processes.hard_limit)]);
    out.insert("open files".to_string(), vec![s(l.max_open_files.soft_limit), s(l.max_open_files.hard_limit)]);
    out.insert("locked memory".to_string(), vec![s(l.max_locked_memory.soft_limit), s(l.max_locked_memory.hard_limit)]);
    out.insert("address space".to_string(), vec![s(l.max_address_space.soft_limit), s(l.max_address_space.hard_limit)]);
    out.insert("file locks".to_string(), vec![s(l.max_file_locks.soft_limit), s(l.max_file_locks.hard_limit)]);
    out.insert("pending signals".to_string(), vec![s(l.max_pending_signals.soft_limit), s(l.max_pending_signals.hard_limit)]);
    out.insert("msgqueue size".to_string(), vec![s(l.max_msgqueue_size.soft_limit), s(l.max_msgqueue_size.hard_limit)]);
    out.insert("nice prio".to_string(), vec![s(l.max_nice_priority.soft_limit), s(l.max_nice_priority.hard_limit)]);
    out.insert("rt prio".to_string(), vec![s(l.max_realtime_priority.soft_limit), s(l.max_realtime_priority.hard_limit)]);
    out.insert("rt timeout".to_string(), vec![s(l.max_realtime_timeout.soft_limit), s(l.max_realtime_timeout.hard_limit)]);

    out
}

/// Run sys.proc
pub fn run(rt: &ModRequest) -> ModResponse {
    let mut res = runtime::new_call_response();
    let cmd = get_arg(rt, "search");
    let mut data: HashMap<String, serde_json::Value> = HashMap::default();

    if cmd.is_empty() {
        res.set_retcode(1);
        res.set_message("Search criteria is not defined");
        return res;
    }

    if let Some(p) = find_process(cmd) {
        if get_opt(rt, "pid") {
            data.insert("pid".to_string(), json!(p.pid()));
        }

        if get_opt(rt, "limits") {
            data.insert("limits".to_string(), json!(get_limits(p)));
        }
    } else {
        res.set_retcode(1);
        res.set_message("Process not found");
        return res;
    }

    // Set payload
    if let Err(err) = res.set_data(&data) {
        res.set_retcode(1);
        res.set_message(&format!("{}", err));
        return res;
    }

    res.set_message("Process is running");
    res
}
