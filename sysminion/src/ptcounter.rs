use procfs::Current;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    time::Instant,
};
use sysinfo::{DiskKind, Disks, System};

#[derive(Debug, Clone)]
struct DiskStats {
    device: String,
    mountpoint: String,

    // absolute counters since boot (kernel)
    read_bytes: u64,
    write_bytes: u64,

    // derived values over last sample window
    read_delta: u64,
    write_delta: u64,
    read_bps: f64,
    write_bps: f64,

    initialized: bool,
}

impl DiskStats {
    fn new(device: String, mountpoint: String) -> Self {
        Self { device, mountpoint, read_bytes: 0, write_bytes: 0, read_delta: 0, write_delta: 0, read_bps: 0.0, write_bps: 0.0, initialized: false }
    }

    fn sample(&mut self, new_read: u64, new_write: u64, dt_secs: f64) {
        if !self.initialized {
            self.read_bytes = new_read;
            self.write_bytes = new_write;
            self.initialized = true;
            self.read_delta = 0;
            self.write_delta = 0;
            self.read_bps = 0.0;
            self.write_bps = 0.0;
            return;
        }

        self.read_delta = new_read.saturating_sub(self.read_bytes);
        self.write_delta = new_write.saturating_sub(self.write_bytes);

        self.read_bytes = new_read;
        self.write_bytes = new_write;

        let dt = dt_secs.max(1e-9);
        self.read_bps = self.read_delta as f64 / dt;
        self.write_bps = self.write_delta as f64 / dt;
    }
}

/// Process and task counter.
/// This is used to keep track of the number of processes and tasks
/// running on the minion, to avoid overloading it.
///
/// Counter is also keeping track of done tasks and sends their cycle IDs
/// back to the master for bookkeeping when it reaches zero. Master can then
/// figure out what tasks were dropped/missed, if any.
#[derive(Debug)]
pub struct PTCounter {
    sys: System,
    disks: Disks,

    // System stats
    cpu_usage: f32,
    active_processes: usize,
    loadaverage: f32,
    disk_stats: Vec<DiskStats>,
    last_stats_ts: Option<Instant>,

    // Minion stats
    tasks: HashSet<String>, // Cycle IDs of added tasks
    done: HashSet<String>,  // Cycle IDs of done tasks
}

impl PTCounter {
    /// Create new counter
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let mut disks = Disks::new();
        disks.refresh(true);

        Self {
            sys,
            tasks: HashSet::new(),
            done: HashSet::new(),
            loadaverage: 0.0,
            cpu_usage: 0.0,
            active_processes: 0,
            disks,
            disk_stats: Vec::new(),
            last_stats_ts: None,
        }
    }

    fn mapper_to_dm_kname(dev: &str) -> Option<String> {
        // dev like "/dev/mapper/ubuntu--vg-root"
        let name = dev.strip_prefix("/dev/mapper/")?;

        // /sys/class/block/dm-*/dm/name contains the mapper name
        let sys_block = Path::new("/sys/class/block");
        for entry in fs::read_dir(sys_block).ok()? {
            let entry = entry.ok()?;
            let fname = entry.file_name();
            let kname = fname.to_string_lossy();
            if !kname.starts_with("dm-") {
                continue;
            }

            let dm_name_path = entry.path().join("dm").join("name");
            if let Ok(dm_name) = fs::read_to_string(dm_name_path)
                && dm_name.trim() == name
            {
                return Some(kname.to_string()); // "dm-0"
            }
        }
        None
    }

    fn device_to_kname(dev: &str) -> Option<String> {
        if dev.starts_with("/dev/mapper/") { Self::mapper_to_dm_kname(dev) } else { Some(dev.strip_prefix("/dev/").unwrap_or(dev).to_string()) }
    }

    fn diskstats_bytes() -> procfs::ProcResult<HashMap<String, (u64, u64)>> {
        let mut map = HashMap::new();
        for ds in procfs::diskstats()? {
            // sectors -> bytes (Linux diskstats sectors are 512-byte units in practice)
            let read_bytes = ds.sectors_read * 512;
            let write_bytes = ds.sectors_written * 512;
            map.insert(ds.name, (read_bytes, write_bytes));
        }
        Ok(map)
    }

    /// Update system stats
    /// Called periodically in the minion in a separate thread to refresh stats
    pub(crate) fn update_stats(&mut self) {
        // loadavg
        if let Ok(la) = procfs::LoadAverage::current() {
            self.loadaverage = la.five;
        }

        // cpu + processes
        self.sys.refresh_cpu_all();
        self.sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        self.cpu_usage = self.sys.global_cpu_usage();
        self.active_processes = self.sys.processes().len();

        // dt for rates
        let now = Instant::now();
        let dt_secs = self.last_stats_ts.map(|t| now.duration_since(t).as_secs_f64()).unwrap_or(0.0);
        self.last_stats_ts = Some(now);

        // Read monotonic disk counters ONCE
        let io = match Self::diskstats_bytes() {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to read /proc/diskstats: {e}");
                return;
            }
        };

        // Refresh disk list (for device names + mountpoints)
        self.disks.refresh(true);

        for d in self.disks.list().iter().filter(|d| matches!(d.kind(), DiskKind::HDD | DiskKind::SSD)) {
            let device = d.name().to_string_lossy().to_string(); // e.g. "/dev/nvme0n1p2" or "/dev/mapper/..."
            let mountpoint = d.mount_point().to_string_lossy().to_string();

            // Try to match device to diskstats key
            let kname = match Self::device_to_kname(&device) {
                Some(k) => k,
                None => continue,
            };

            let (new_read, new_write) = match io.get(&kname) {
                Some(v) => *v,
                None => continue,
            };

            match self.disk_stats.iter_mut().find(|ds| ds.device == device) {
                Some(ds) => ds.sample(new_read, new_write, dt_secs),
                None => {
                    let mut ds = DiskStats::new(device, mountpoint);
                    ds.sample(new_read, new_write, dt_secs);
                    self.disk_stats.push(ds);
                }
            }
        }

        // Top writers without cloning everything like a potato
        let mut top: Vec<&DiskStats> = self.disk_stats.iter().collect();
        top.sort_by(|a, b| b.write_bps.partial_cmp(&a.write_bps).unwrap_or(std::cmp::Ordering::Equal));

        log::debug!(
            "Stats: loadavg(5m)={:.2}, cpu={:.1}%, procs={}, top_writers={:#?}",
            self.loadaverage,
            self.cpu_usage,
            self.active_processes,
            top.into_iter().take(3).collect::<Vec<_>>()
        );
    }

    /// Increment task counter
    pub fn inc(&mut self, cid: &str) {
        self.tasks.insert(cid.to_string());
        log::info!("Added task {}, count increased to {}, load average: {}", cid, self.tasks.len(), self.loadaverage);
    }

    /// Decrement task counter
    pub fn dec(&mut self, cid: &str) {
        self.tasks.remove(cid);
        self.done.insert(cid.to_string());
        log::info!("Removed task {}, count decreased to {}, load average: {}", cid, self.tasks.len(), self.loadaverage);
    }

    /// Get current task count
    pub fn is_done(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get done task ids
    /// This also clears the done set.
    pub fn get_done(&mut self) -> Vec<String> {
        self.done.iter().cloned().collect::<Vec<String>>()
    }

    pub fn flush_done(&mut self) {
        self.done.clear();
    }

    /// Get current load average (5 min)
    pub fn get_loadaverage(&self) -> f32 {
        self.loadaverage
    }

    /// Get current CPU usage (%)
    pub fn get_cpu_usage(&self) -> f32 {
        self.cpu_usage
    }

    /// Get current active process count
    pub fn get_active_processes(&self) -> usize {
        self.active_processes
    }

    /// Current write pressure of this minion (bytes/sec).
    /// Picks the "/" mount if present; otherwise falls back to max writer.
    pub fn get_io_bps(&self) -> f64 {
        if let Some(root) = self.disk_stats.iter().find(|ds| ds.mountpoint == "/") {
            return root.write_bps;
        }
        self.disk_stats.iter().map(|ds| ds.write_bps).fold(0.0, f64::max)
    }
}
