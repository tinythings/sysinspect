use crate::console::{ConsoleMinionTopDisk, ConsoleMinionTopInterface, ConsoleMinionTopProcess, ConsoleMinionTopRequest, ConsoleMinionTopSnapshot};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use sysinfo::{Components, Disks, Networks, ProcessesToUpdate, System};

#[derive(Debug, Default, Clone, Copy)]
struct InterfaceCounters {
    rx_total_bytes: u64,
    tx_total_bytes: u64,
}

#[derive(Debug, Default)]
struct NetworkCounters {
    rx_total_bytes: u64,
    tx_total_bytes: u64,
    interfaces: BTreeMap<String, InterfaceCounters>,
}

pub fn collect_top_snapshot(minion_id: &str, request: &ConsoleMinionTopRequest) -> ConsoleMinionTopSnapshot {
    let sample_interval = Duration::from_millis(200);
    let mut system = System::new_all();
    system.refresh_memory();
    system.refresh_cpu_all();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let mut networks = Networks::new_with_refreshed_list();
    networks.refresh(true);
    let network_before = collect_network_counters(&networks);
    let network_sample_started = Instant::now();

    std::thread::sleep(sample_interval);
    system.refresh_cpu_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    networks.refresh(true);
    let network_elapsed = network_sample_started.elapsed().max(Duration::from_millis(1));
    let network_after = collect_network_counters(&networks);

    let components = Components::new_with_refreshed_list();

    let mut disks = Disks::new();
    disks.refresh(true);

    let mut processes: Vec<ConsoleMinionTopProcess> = system
        .processes()
        .values()
        .map(|process| {
            let command = if !process.cmd().is_empty() {
                process.cmd().iter().map(|part| part.to_string_lossy()).collect::<Vec<_>>().join(" ")
            } else if let Some(exe) = process.exe() {
                exe.display().to_string()
            } else {
                process.name().to_string_lossy().into_owned()
            };

            ConsoleMinionTopProcess {
                pid: process.pid().as_u32(),
                name: process.name().to_string_lossy().into_owned(),
                command,
                user: process.user_id().map(|uid| uid.to_string()).unwrap_or_else(|| "-".to_string()),
                threads: 0,
                cpu_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
            }
        })
        .collect();
    processes.sort_by(|a, b| {
        b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal).then_with(|| b.memory_bytes.cmp(&a.memory_bytes))
    });
    processes.truncate(request.process_limit.max(1));

    let mut disk_rows: Vec<ConsoleMinionTopDisk> = disks
        .list()
        .iter()
        .map(|disk| {
            let total_bytes = disk.total_space();
            let available_bytes = disk.available_space();
            let used_bytes = total_bytes.saturating_sub(available_bytes);
            let used_percent = if total_bytes == 0 { 0.0 } else { (used_bytes as f64 / total_bytes as f64 * 100.0) as f32 };
            ConsoleMinionTopDisk {
                name: disk.name().to_string_lossy().into_owned(),
                mount_point: disk.mount_point().to_string_lossy().into_owned(),
                total_bytes,
                available_bytes,
                used_bytes,
                used_percent,
            }
        })
        .collect();
    disk_rows.sort_by(|a, b| b.used_percent.partial_cmp(&a.used_percent).unwrap_or(std::cmp::Ordering::Equal));

    let network_rx_total_bytes = network_after.rx_total_bytes;
    let network_tx_total_bytes = network_after.tx_total_bytes;
    let network_rx_rate_bytes_per_sec =
        rate_bytes_per_sec(network_after.rx_total_bytes.saturating_sub(network_before.rx_total_bytes), network_elapsed);
    let network_tx_rate_bytes_per_sec =
        rate_bytes_per_sec(network_after.tx_total_bytes.saturating_sub(network_before.tx_total_bytes), network_elapsed);
    let mut network_interfaces: Vec<ConsoleMinionTopInterface> = network_after
        .interfaces
        .iter()
        .map(|(name, counters)| {
            let before = network_before.interfaces.get(name).copied().unwrap_or_default();
            ConsoleMinionTopInterface {
                name: name.to_string(),
                rx_total_bytes: counters.rx_total_bytes,
                tx_total_bytes: counters.tx_total_bytes,
                rx_rate_bytes_per_sec: rate_bytes_per_sec(counters.rx_total_bytes.saturating_sub(before.rx_total_bytes), network_elapsed),
                tx_rate_bytes_per_sec: rate_bytes_per_sec(counters.tx_total_bytes.saturating_sub(before.tx_total_bytes), network_elapsed),
            }
        })
        .collect();
    network_interfaces.sort_by(|a, b| {
        b.rx_total_bytes.saturating_add(b.tx_total_bytes).cmp(&a.rx_total_bytes.saturating_add(a.tx_total_bytes)).then_with(|| a.name.cmp(&b.name))
    });
    let load_avg = System::load_average();
    let cpu_temp_celsius = components
        .list()
        .iter()
        .filter_map(|component| {
            let label = component.label().to_ascii_lowercase();
            (label.contains("cpu") || label.contains("core")).then(|| component.temperature()).flatten()
        })
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    ConsoleMinionTopSnapshot {
        minion_id: minion_id.to_string(),
        hostname: System::host_name().unwrap_or_else(|| minion_id.to_string()),
        uptime_secs: System::uptime(),
        load_avg_one: load_avg.one as f32,
        load_avg_five: load_avg.five as f32,
        load_avg_fifteen: load_avg.fifteen as f32,
        cpu_percent: system.global_cpu_usage(),
        cpu_temp_celsius,
        cpu_per_core: system.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(),
        memory_total_bytes: system.total_memory(),
        memory_used_bytes: system.used_memory(),
        memory_available_bytes: system.available_memory(),
        swap_total_bytes: system.total_swap(),
        swap_used_bytes: system.used_swap(),
        network_rx_total_bytes,
        network_tx_total_bytes,
        network_rx_rate_bytes_per_sec,
        network_tx_rate_bytes_per_sec,
        network_interfaces,
        disks: disk_rows,
        processes,
    }
}

fn collect_network_counters(networks: &Networks) -> NetworkCounters {
    let mut snapshot = NetworkCounters::default();
    for (name, net) in networks.iter() {
        let counters = InterfaceCounters { rx_total_bytes: net.total_received(), tx_total_bytes: net.total_transmitted() };
        snapshot.rx_total_bytes = snapshot.rx_total_bytes.saturating_add(counters.rx_total_bytes);
        snapshot.tx_total_bytes = snapshot.tx_total_bytes.saturating_add(counters.tx_total_bytes);
        snapshot.interfaces.insert(name.to_string(), counters);
    }
    snapshot
}

fn rate_bytes_per_sec(delta_bytes: u64, elapsed: Duration) -> u64 {
    if delta_bytes == 0 {
        return 0;
    }
    (delta_bytes as f64 / elapsed.as_secs_f64()) as u64
}
