use libmodcore::{
    response::ModResponse,
    runtime::{self, ModRequest},
};
use serde_json::json;
use std::{collections::HashMap, vec};
#[cfg(target_os = "freebsd")]
use std::process::Command;

#[cfg(target_os = "linux")]
use procfs::process::{LimitValue, Process, all_processes};
#[cfg(target_os = "freebsd")]
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[cfg(target_os = "linux")]
fn find_process(cmd: &str) -> Option<Process> {
    all_processes()
        .ok()?
        .flatten()
        .find(|process| process.cmdline().is_ok_and(|cmdline| cmdline.join(" ").starts_with(cmd)))
}

#[cfg(target_os = "linux")]
fn get_limits(process: &Process) -> HashMap<String, Vec<serde_json::Value>> {
    fn s(limit: LimitValue) -> serde_json::Value {
        match limit {
            LimitValue::Unlimited => json!("unlimited"),
            LimitValue::Value(value) => json!(value),
        }
    }

    process.limits().map_or_else(
        |_| HashMap::default(),
        |limits| {
            [
                ("cpu time", vec![s(limits.max_cpu_time.soft_limit), s(limits.max_cpu_time.hard_limit)]),
                ("file size", vec![s(limits.max_file_size.soft_limit), s(limits.max_file_size.hard_limit)]),
                ("data size", vec![s(limits.max_data_size.soft_limit), s(limits.max_data_size.hard_limit)]),
                ("stack size", vec![s(limits.max_stack_size.soft_limit), s(limits.max_stack_size.hard_limit)]),
                (
                    "core file size",
                    vec![s(limits.max_core_file_size.soft_limit), s(limits.max_core_file_size.hard_limit)],
                ),
                (
                    "resident set",
                    vec![s(limits.max_resident_set.soft_limit), s(limits.max_resident_set.hard_limit)],
                ),
                ("processes", vec![s(limits.max_processes.soft_limit), s(limits.max_processes.hard_limit)]),
                ("open files", vec![s(limits.max_open_files.soft_limit), s(limits.max_open_files.hard_limit)]),
                (
                    "locked memory",
                    vec![s(limits.max_locked_memory.soft_limit), s(limits.max_locked_memory.hard_limit)],
                ),
                (
                    "address space",
                    vec![s(limits.max_address_space.soft_limit), s(limits.max_address_space.hard_limit)],
                ),
                ("file locks", vec![s(limits.max_file_locks.soft_limit), s(limits.max_file_locks.hard_limit)]),
                (
                    "pending signals",
                    vec![s(limits.max_pending_signals.soft_limit), s(limits.max_pending_signals.hard_limit)],
                ),
                (
                    "msgqueue size",
                    vec![s(limits.max_msgqueue_size.soft_limit), s(limits.max_msgqueue_size.hard_limit)],
                ),
                (
                    "nice prio",
                    vec![s(limits.max_nice_priority.soft_limit), s(limits.max_nice_priority.hard_limit)],
                ),
                (
                    "rt prio",
                    vec![s(limits.max_realtime_priority.soft_limit), s(limits.max_realtime_priority.hard_limit)],
                ),
                (
                    "rt timeout",
                    vec![s(limits.max_realtime_timeout.soft_limit), s(limits.max_realtime_timeout.hard_limit)],
                ),
            ]
            .into_iter()
            .map(|(name, values)| (name.to_string(), values))
            .collect()
        },
    )
}

#[cfg(target_os = "freebsd")]
fn proc_value(value: &str) -> serde_json::Value {
    if value.eq_ignore_ascii_case("infinity") || value.eq_ignore_ascii_case("unlimited") {
        json!("unlimited")
    } else {
        value.parse::<u64>().map_or_else(|_| json!(value), |number| json!(number))
    }
}

#[cfg(target_os = "freebsd")]
fn find_process(cmd: &str) -> Option<(u32, String)> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything()
            .without_cpu()
            .without_disk_usage()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always),
    );

    system
        .processes()
        .values()
        .find_map(|process| {
            Some(
                process
                    .cmd()
                    .iter()
                    .map(|part| part.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .filter(|joined| !joined.is_empty())
            .or_else(|| process.exe().map(|path| path.display().to_string()))
            .or_else(|| Some(process.name().to_string_lossy().into_owned()))
            .filter(|command| command.starts_with(cmd))
            .map(|command| (process.pid().as_u32(), command))
        })
}

#[cfg(target_os = "freebsd")]
fn get_limits(pid: u32) -> HashMap<String, Vec<serde_json::Value>> {
    Command::new("procstat")
        .arg("-r")
        .arg(pid.to_string())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .map(|stdout| {
            stdout
                .lines()
                .filter_map(|line| {
                    let fields = line.split_whitespace().collect::<Vec<_>>();
                    fields
                        .first()
                        .and_then(|field| field.parse::<u32>().ok())
                        .filter(|found_pid| *found_pid == pid)
                        .and_then(|_| {
                            (fields.len() >= 5).then(|| {
                                (
                                    fields[2..fields.len() - 2].join(" "),
                                    vec![proc_value(fields[fields.len() - 2]), proc_value(fields[fields.len() - 1])],
                                )
                            })
                        })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn run(rt: &ModRequest) -> ModResponse {
    let mut response = runtime::new_call_response();
    let search = runtime::get_arg(rt, "search");
    let mut data: HashMap<String, serde_json::Value> = HashMap::default();

    if search.is_empty() {
        response.set_retcode(1);
        response.set_message("Search criteria is not defined");
        return response;
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(process) = find_process(&search) {
            if runtime::get_opt(rt, "pid") {
                data.insert("pid".to_string(), json!(process.pid()));
            }

            if runtime::get_opt(rt, "limits") {
                data.insert("limits".to_string(), json!(get_limits(&process)));
            }
        } else {
            response.set_retcode(1);
            response.set_message("Process not found");
            return response;
        }
    }

    #[cfg(target_os = "freebsd")]
    {
        if let Some((pid, _command)) = find_process(&search) {
            if runtime::get_opt(rt, "pid") {
                data.insert("pid".to_string(), json!(pid));
            }

            if runtime::get_opt(rt, "limits") {
                data.insert("limits".to_string(), json!(get_limits(pid)));
            }
        } else {
            response.set_retcode(1);
            response.set_message("Process not found");
            return response;
        }
    }

    if let Err(err) = response.set_data(&data) {
        response.set_retcode(1);
        response.set_message(&format!("{err}"));
        return response;
    }

    response.set_message("Process is running");
    response
}
