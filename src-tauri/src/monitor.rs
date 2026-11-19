// Readings of the machine while a game runs: the processor, the graphics
// card, the memory and the temperatures, once a second.
//
// Everything comes from /proc and /sys, so the program needs no extra crate.
// nvidia-smi is asked only when the card is NVIDIA, because the driver does
// not offer the numbers anywhere else.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const TICK: &str = "monitor:tick";

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    pub usage: Option<f32>,
    pub temp: Option<f32>,
    pub clock_mhz: Option<f32>,
    pub mem_used_mb: Option<u64>,
    pub mem_total_mb: Option<u64>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Sensor {
    pub chip: String,
    pub label: String,
    pub value: f32,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub ram_mb: u64,
}

/// The battery of a laptop: its charge and whether it is on the mains.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub percent: f32,
    /// On the mains: charging, full or held at a limit. Off it, discharging.
    pub plugged: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub battery: Option<Battery>,
    pub cpu: f32,
    pub cores: Vec<f32>,
    pub cpu_name: String,
    pub cpu_cores: u32,
    pub cpu_threads: u32,
    pub cpu_mhz: Option<f32>,
    pub governor: Option<String>,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub ram_available_mb: u64,
    pub swap_used_mb: u64,
    pub swap_total_mb: u64,
    pub cpu_temp: Option<f32>,
    pub cpu_watts: Option<f32>,
    pub temps: Vec<Sensor>,
    pub fans: Vec<Sensor>,
    pub voltages: Vec<Sensor>,
    pub powers: Vec<Sensor>,
    pub gpu: Option<Gpu>,
    pub net_rx_kbps: f32,
    pub net_tx_kbps: f32,
    pub disk_read_kbps: f32,
    pub disk_write_kbps: f32,
    pub fps: Option<f32>,
    pub processes: Vec<Process>,
}

static LATEST: Mutex<Option<Snapshot>> = Mutex::new(None);

pub fn latest() -> Option<Snapshot> {
    LATEST.lock().ok().and_then(|slot| slot.clone())
}

// ── Processor ───────────────────────────────────────────────────────────────

/// The model of the processor and how many cores and threads it has, as
/// /proc/cpuinfo tells it. It does not change while the program runs.
struct CpuModel {
    name: String,
    cores: u32,
    threads: u32,
}

fn cpu_model() -> &'static CpuModel {
    static MODEL: OnceLock<CpuModel> = OnceLock::new();
    MODEL.get_or_init(|| {
        let text = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let name = text
            .lines()
            .find(|line| line.starts_with("model name"))
            .and_then(|line| line.split(':').nth(1))
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let threads = text.lines().filter(|line| line.starts_with("processor")).count() as u32;
        let mut core_ids = std::collections::HashSet::new();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("core id") {
                if let Some(id) = rest.split(':').nth(1) {
                    core_ids.insert(id.trim().to_string());
                }
            }
        }
        let cores = if core_ids.is_empty() { threads } else { core_ids.len() as u32 };
        CpuModel { name, cores, threads }
    })
}

/// One core as /proc/stat counts it: work and idle ticks in USER_HZ.
type CpuTicks = (u64, u64);

/// The time the processor has spent working and idle, per core, as /proc/stat
/// counts it in USER_HZ.
fn read_cpu_times() -> (Vec<CpuTicks>, Vec<CpuTicks>) {
    let text = fs::read_to_string("/proc/stat").unwrap_or_default();
    let mut total = Vec::new();
    let mut aggregate = (0, 0);
    for line in text.lines() {
        if !line.starts_with("cpu") {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        let values: Vec<u64> = fields[1..].iter().filter_map(|v| v.parse().ok()).collect();
        if values.len() < 4 {
            continue;
        }
        let idle = values[3] + values.get(4).copied().unwrap_or(0);
        let all: u64 = values.iter().sum();
        if fields[0] == "cpu" {
            aggregate = (idle, all);
        } else {
            total.push((idle, all));
        }
    }
    (vec![aggregate], total)
}

fn usage_between(before: (u64, u64), after: (u64, u64)) -> f32 {
    let idle = after.0.saturating_sub(before.0);
    let all = after.1.saturating_sub(before.1);
    if all == 0 {
        0.0
    } else {
        ((all - idle) as f32 * 100.0 / all as f32).clamp(0.0, 100.0)
    }
}

// ── Memory ──────────────────────────────────────────────────────────────────

fn meminfo_kb(key: &str) -> u64 {
    let text = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            let number: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
            return number.parse().unwrap_or(0);
        }
    }
    0
}

// ── Temperatures ────────────────────────────────────────────────────────────

/// Every sensor of one kind that the drivers list: `temp` for degrees, `fan`
/// for revolutions per minute, `in` for volts and `power` for watts.
fn read_sensors(kind: &str) -> Vec<Sensor> {
    let mut found = Vec::new();
    let Ok(chips) = fs::read_dir("/sys/class/hwmon") else { return found };
    for chip in chips.flatten() {
        let dir = chip.path();
        let chip_name = fs::read_to_string(dir.join("name")).unwrap_or_default().trim().to_string();
        let Ok(inputs) = fs::read_dir(&dir) else { continue };
        for input in inputs.flatten() {
            let file = input.file_name().to_string_lossy().to_string();
            let Some(rest) = file.strip_prefix(kind) else { continue };
            // Watts are offered as the last reading or as the average of a
            // window, depending on the chip.
            let index = match rest.strip_suffix("_input") {
                Some(index) => index,
                None if kind == "power" => match rest.strip_suffix("_average") {
                    Some(index) => index,
                    None => continue,
                },
                None => continue,
            };
            let Some(raw) = read_number::<f32>(&input.path()) else { continue };
            // Temperatures come in thousandths of a degree, volts in
            // thousandths of a volt, watts in millionths of a watt; turns come
            // as they are.
            let value = match kind {
                "temp" | "in" => raw / 1000.0,
                "power" => raw / 1_000_000.0,
                _ => raw,
            };
            if !value.is_finite() || value <= 0.0 {
                continue;
            }
            let label = fs::read_to_string(dir.join(format!("{kind}{index}_label")))
                .map(|text| text.trim().to_string())
                .unwrap_or_else(|_| format!("{kind} {index}"));
            found.push(Sensor { chip: chip_name.clone(), label, value });
        }
    }
    found.sort_by(|a, b| a.chip.cmp(&b.chip).then_with(|| a.label.cmp(&b.label)));
    found
}

/// The temperature of the processor, taken from the sensors already read: the
/// one of the chip that reports the package, else the warmest of the others.
fn cpu_temp_from(temps: &[Sensor]) -> Option<f32> {
    temps
        .iter()
        .find(|sensor| {
            sensor.chip.contains("k10temp") || sensor.chip.contains("coretemp") || sensor.chip.contains("zenpower")
        })
        .map(|sensor| sensor.value)
        .or_else(|| {
            temps
                .iter()
                .filter(|sensor| sensor.chip.contains("cpu") || sensor.chip.contains("pkg"))
                .map(|sensor| sensor.value)
                .reduce(f32::max)
        })
}

/// The watts the processor draws, from the same chip that reports its
/// temperature.
fn cpu_watts_from(powers: &[Sensor]) -> Option<f32> {
    powers
        .iter()
        .find(|sensor| {
            sensor.chip.contains("k10temp") || sensor.chip.contains("coretemp") || sensor.chip.contains("zenpower")
        })
        .map(|sensor| sensor.value)
}

// ── Graphics card ───────────────────────────────────────────────────────────

fn read_number<T: std::str::FromStr>(path: &Path) -> Option<T> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn card_temp(device: &Path) -> Option<f32> {
    let chips = fs::read_dir(device.join("hwmon")).ok()?;
    for chip in chips.flatten() {
        if let Some(milli) = read_number::<f32>(&chip.path().join("temp1_input")) {
            if milli > 0.0 {
                return Some(milli / 1000.0);
            }
        }
    }
    None
}

/// The name lspci gives the graphics card, which is the only place a virtual
/// machine names it ("VMware SVGA II" and the like). It does not change while
/// the program runs.
fn gpu_name_from_lspci() -> &'static str {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        let text = Command::new("lspci")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default();
        for line in text.lines() {
            let lower = line.to_lowercase();
            if !["vga", "3d", "display"].iter().any(|key| lower.contains(key)) {
                continue;
            }
            let name = if lower.contains("vmware") {
                "VMware SVGA II".to_string()
            } else if lower.contains("virtualbox") || lower.contains("vbox") {
                "VirtualBox VGA".to_string()
            } else if lower.contains("virtio") {
                "VirtIO GPU".to_string()
            } else if lower.contains("qxl") || lower.contains("red hat") {
                "QXL (QEMU/KVM)".to_string()
            } else {
                // The name of the card is the part in brackets, when lspci
                // prints one; else what follows the class of the device.
                match (line.find('['), line.rfind(']')) {
                    (Some(start), Some(end)) if end > start => line[start + 1..end].to_string(),
                    _ => line.split(": ").nth(1).unwrap_or(line).trim().to_string(),
                }
            };
            return name;
        }
        String::new()
    })
}

fn card_name(device: &Path) -> String {
    // The vendor of the card says more than its PCI identifier: a virtual
    // machine shows up here as VMware, QEMU or virtio.
    if let Some(vendor) = vendor_of(device) {
        let known = match vendor {
            0x15ad => Some("VMware SVGA"),
            0x1234 => Some("QEMU VGA"),
            0x1af4 => Some("Virtio GPU"),
            0x1002 => Some("AMD"),
            0x10de => Some("NVIDIA"),
            0x8086 => Some("Intel"),
            _ => None,
        };
        if let Some(name) = known {
            return name.to_string();
        }
    }
    if let Ok(text) = fs::read_to_string(device.join("uevent")) {
        for line in text.lines() {
            if let Some(value) = line.strip_prefix("DRIVER=") {
                return value.trim().to_string();
            }
        }
    }
    "GPU".to_string()
}

fn vendor_of(device: &Path) -> Option<u32> {
    fs::read_to_string(device.join("vendor")).ok().and_then(|v| {
        u32::from_str_radix(v.trim().trim_start_matches("0x"), 16).ok()
    })
}

fn drm_cards() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else { return Vec::new() };
    let mut cards: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().starts_with("card") && !name.to_string_lossy().contains('-'))
                .unwrap_or(false)
        })
        .collect();
    cards.sort();
    cards
}

/// The clock the card reports now. AMD keeps a table of states and marks the
/// one in use; Intel gives the current figure in one file.
fn card_clock(device: &Path) -> Option<f32> {
    if let Ok(table) = fs::read_to_string(device.join("pp_dpm_sclk")) {
        if let Some(line) = table.lines().find(|line| line.contains('*')) {
            let number: String = line
                .chars()
                .skip_while(|one| !one.is_ascii_digit())
                .take_while(|one| one.is_ascii_digit() || *one == '.')
                .collect();
            if let Some(mhz) = number.parse::<f32>().ok().filter(|value| *value > 0.0) {
                return Some(mhz);
            }
        }
    }
    read_number::<f32>(&device.join("gt_cur_freq_mhz")).filter(|value| *value > 0.0)
}

fn amd(device: &Path) -> Option<Gpu> {
    let usage = read_number::<f32>(&device.join("gpu_busy_percent"));
    let used = read_number::<u64>(&device.join("mem_info_vram_used")).map(|bytes| bytes / 1_048_576);
    let total = read_number::<u64>(&device.join("mem_info_vram_total")).map(|bytes| bytes / 1_048_576);
    if usage.is_none() && used.is_none() {
        return None;
    }
    Some(Gpu {
        name: card_name(device),
        usage,
        temp: card_temp(device),
        clock_mhz: card_clock(device),
        mem_used_mb: used,
        mem_total_mb: total,
    })
}

fn intel(device: &Path) -> Option<Gpu> {
    // The integrated card has no use figure. A frequency node is what tells it
    // apart from a card that offers nothing at all, so it is read only to be
    // checked.
    let _freq = read_number::<u64>(&device.join("gt_cur_freq_mhz"))?;
    Some(Gpu {
        name: card_name(device),
        usage: None,
        temp: card_temp(device),
        clock_mhz: card_clock(device),
        mem_used_mb: None,
        mem_total_mb: None,
    })
}

/// A card of a make that offers no use figure here, a virtual machine for one:
/// the name is still worth showing.
fn generic(device: &Path) -> Option<Gpu> {
    Some(Gpu {
        name: card_name(device),
        usage: None,
        temp: card_temp(device),
        clock_mhz: card_clock(device),
        mem_used_mb: None,
        mem_total_mb: None,
    })
}

fn nvidia() -> Option<Gpu> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,temperature.gpu,memory.used,memory.total,clocks.gr",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next()?.trim().to_string();
    let fields: Vec<&str> = first.split(',').map(str::trim).collect();
    if fields.is_empty() {
        return None;
    }
    let number = |index: usize| -> Option<f32> { fields.get(index).and_then(|v| v.parse().ok()) };
    Some(Gpu {
        name: fields[0].to_string(),
        usage: number(1),
        temp: number(2),
        clock_mhz: number(5),
        mem_used_mb: number(3).map(|v| v as u64),
        mem_total_mb: number(4).map(|v| v as u64),
    })
}

/// The name lspci gives the card replaces the one guessed from /sys, which for
/// a virtual machine is only "VMware SVGA".
fn named(gpu: Gpu) -> Gpu {
    let from_lspci = gpu_name_from_lspci();
    if from_lspci.is_empty() {
        gpu
    } else {
        Gpu { name: from_lspci.to_string(), ..gpu }
    }
}

fn read_gpu() -> Option<Gpu> {
    for card in drm_cards() {
        let device = card.join("device");
        match vendor_of(&device) {
            Some(0x1002) => {
                if let Some(gpu) = amd(&device) {
                    return Some(named(gpu));
                }
            }
            Some(0x10de) => {
                if let Some(gpu) = nvidia() {
                    return Some(gpu);
                }
            }
            Some(0x8086) => {
                if let Some(gpu) = intel(&device) {
                    return Some(named(gpu));
                }
            }
            _ => {
                if let Some(gpu) = generic(&device) {
                    return Some(named(gpu));
                }
            }
        }
    }
    nvidia()
}

// ── Clock, network, disk and processes ─────────────────────────────────────

/// The interface packets leave by: the one of the default route.
fn default_interface() -> Option<String> {
    let text = fs::read_to_string("/proc/net/route").ok()?;
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // The destination of the default route is 00000000.
        if fields.len() > 2 && fields[1] == "00000000" {
            return Some(fields[0].to_string());
        }
    }
    None
}

/// Bytes received and sent by an interface, as /sys counts them.
fn interface_bytes(interface: &str) -> (u64, u64) {
    let base = Path::new("/sys/class/net").join(interface).join("statistics");
    let read = |name: &str| read_number::<u64>(&base.join(name)).unwrap_or(0);
    (read("rx_bytes"), read("tx_bytes"))
}

/// Sectors read and written by the disks, as /proc/diskstats counts them.
fn disk_sectors() -> (u64, u64) {
    let text = fs::read_to_string("/proc/diskstats").unwrap_or_default();
    let (mut read, mut written) = (0u64, 0u64);
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }
        let name = fields[2];
        // Partitions would count the same work twice, and loop devices are not
        // disks of the machine.
        if name.starts_with("loop") || name.starts_with("ram") || name.chars().last().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        read += fields[5].parse::<u64>().unwrap_or(0);
        written += fields[9].parse::<u64>().unwrap_or(0);
    }
    (read, written)
}

/// The processor speed the kernel reports now, averaged over the cores.
fn cpu_mhz() -> Option<f32> {
    let text = fs::read_to_string("/proc/cpuinfo").ok()?;
    let values: Vec<f32> = text
        .lines()
        .filter(|line| line.starts_with("cpu MHz"))
        .filter_map(|line| line.split(':').nth(1))
        .filter_map(|value| value.trim().parse().ok())
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f32>() / values.len() as f32)
    }
}

fn governor() -> Option<String> {
    let text = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").ok()?;
    let name = text.trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// What each process has used, as (its processor time in ticks, its memory in
/// pages), kept between readings to tell what it did in the last second.
type ProcessTimes = std::collections::HashMap<u32, (u64, u64)>;

fn read_processes(previous: &mut ProcessTimes, elapsed: f32) -> Vec<Process> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else { return found };
    let page_kb = 4u64;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(pid) = name.parse::<u32>() else { continue };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else { continue };
        // The name of the process is between the first pair of parentheses and
        // can hold spaces; the fields after it are plain numbers.
        let Some(open) = stat.find('(') else { continue };
        let Some(close) = stat.rfind(')') else { continue };
        let comm = stat[open + 1..close].to_string();
        let rest: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        if rest.len() < 22 {
            continue;
        }
        let ticks: u64 = rest[11].parse::<u64>().unwrap_or(0) + rest[12].parse::<u64>().unwrap_or(0);
        let pages: u64 = rest[21].parse::<u64>().unwrap_or(0);
        let (before, _) = previous.get(&pid).copied().unwrap_or((ticks, pages));
        let spent = ticks.saturating_sub(before) as f32;
        // The kernel counts a hundred ticks per second.
        let cpu = if elapsed > 0.0 { (spent / 100.0) * 100.0 / elapsed } else { 0.0 };
        previous.insert(pid, (ticks, pages));
        found.push(Process { pid, name: comm, cpu, ram_mb: pages * page_kb / 1024 });
    }
    found.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    found
}

/// The first battery of the machine, as the kernel reports it. Desktops have
/// none, and a mouse or a pad with a battery is not of the system.
fn read_battery() -> Option<Battery> {
    battery_in(Path::new("/sys/class/power_supply"))
}

fn battery_in(root: &Path) -> Option<Battery> {
    let mut supplies: Vec<PathBuf> = fs::read_dir(root).ok()?.flatten().map(|entry| entry.path()).collect();
    supplies.sort();
    supplies.into_iter().find_map(|supply| {
        let kind = fs::read_to_string(supply.join("type")).ok()?;
        let scope = fs::read_to_string(supply.join("scope")).unwrap_or_default();
        if kind.trim() != "Battery" || scope.trim() == "Device" {
            return None;
        }
        let percent: f32 = read_number(&supply.join("capacity"))?;
        let status = fs::read_to_string(supply.join("status")).unwrap_or_default();
        Some(Battery { percent, plugged: status.trim() != "Discharging" })
    })
}

/// Frames per second written by MangoHud in its log, when the user asked for
/// one in its configuration (`output_folder`): the newest file of the folder,
/// as long as a game is still writing it.
fn mangohud_fps() -> Option<f32> {
    // The configuration is read every time: the log can be turned on from the
    // page of MangoHud while the program runs.
    let path = crate::mangohud::path()?;
    let document = crate::mangohud::read(&path).ok()?;
    let folder = crate::mangohud::log_folder(&document)?;
    fps_in_log(&crate::mangohud::live_log(&folder)?)
}

/// The last frames per second of a log of MangoHud. The log grows during the
/// whole game, so only its start (where the names of the columns are) and its
/// end are read. The names are looked for among the first lines: MangoHud
/// writes a line about the system before them.
fn fps_in_log(log: &Path) -> Option<f32> {
    use std::io::{Read, Seek, SeekFrom};
    const PIECE: u64 = 8 * 1024;
    let mut file = fs::File::open(log).ok()?;
    let size = file.metadata().ok()?.len();

    let mut start = vec![0u8; PIECE.min(size) as usize];
    file.read_exact(&mut start).ok()?;
    let start = String::from_utf8_lossy(&start);
    let header = start.lines().take(10).find(|line| {
        line.split([',', '\t']).any(|column| column.trim().eq_ignore_ascii_case("fps"))
    })?;
    let separator = if header.contains(',') { ',' } else { '\t' };
    let index = header.split(separator).position(|column| column.trim().eq_ignore_ascii_case("fps"))?;

    file.seek(SeekFrom::Start(size.saturating_sub(PIECE))).ok()?;
    let mut end = Vec::new();
    file.read_to_end(&mut end).ok()?;
    let end = String::from_utf8_lossy(&end);
    // The last whole line with a number in that column.
    end.lines()
        .rev()
        .filter(|line| *line != header)
        .find_map(|line| line.split(separator).nth(index)?.trim().parse::<f32>().ok())
}

// ── Sampler ─────────────────────────────────────────────────────────────────

/// What /proc/stat says: the aggregate line and one entry per core.
type CpuTimes = (Vec<(u64, u64)>, Vec<(u64, u64)>);

/// Everything a reading needs to know of the one before it.
struct Sampler {
    cpu: CpuTimes,
    interface: Option<String>,
    net: (u64, u64),
    disk: (u64, u64),
    processes: ProcessTimes,
    at: std::time::Instant,
}

impl Sampler {
    fn new() -> Self {
        let interface = default_interface();
        Self {
            cpu: read_cpu_times(),
            net: interface.as_deref().map(interface_bytes).unwrap_or((0, 0)),
            interface,
            disk: disk_sectors(),
            processes: ProcessTimes::new(),
            at: std::time::Instant::now(),
        }
    }

    fn read(&mut self) -> Snapshot {
        let elapsed = self.at.elapsed().as_secs_f32().max(0.001);
        self.at = std::time::Instant::now();

        let model = cpu_model();
        let (aggregate, cores) = read_cpu_times();
        let cpu = usage_between(self.cpu.0[0], aggregate[0]);
        let core_usage = cores
            .iter()
            .enumerate()
            .map(|(index, after)| self.cpu.1.get(index).map(|b| usage_between(*b, *after)).unwrap_or(0.0))
            .collect();
        self.cpu = (aggregate, cores);

        // A cable or a wireless link can come and go: the interface is asked
        // for again when the one in use disappears.
        if self.interface.is_none() {
            self.interface = default_interface();
        }
        let net = self.interface.as_deref().map(interface_bytes).unwrap_or((0, 0));
        let net_rx = (net.0.saturating_sub(self.net.0)) as f32 / 1024.0 / elapsed;
        let net_tx = (net.1.saturating_sub(self.net.1)) as f32 / 1024.0 / elapsed;
        self.net = net;

        let disk = disk_sectors();
        let disk_read = (disk.0.saturating_sub(self.disk.0)) as f32 * 512.0 / 1024.0 / elapsed;
        let disk_write = (disk.1.saturating_sub(self.disk.1)) as f32 * 512.0 / 1024.0 / elapsed;
        self.disk = disk;

        let processes = read_processes(&mut self.processes, elapsed);
        let temps = read_sensors("temp");
        let fans = read_sensors("fan");
        let voltages = read_sensors("in");
        let powers = read_sensors("power");

        let ram_total = meminfo_kb("MemTotal:");
        let ram_free = meminfo_kb("MemAvailable:");
        let swap_total = meminfo_kb("SwapTotal:");
        let swap_free = meminfo_kb("SwapFree:");

        Snapshot {
            battery: read_battery(),
            cpu,
            cores: core_usage,
            cpu_name: model.name.clone(),
            cpu_cores: model.cores,
            cpu_threads: model.threads,
            cpu_mhz: cpu_mhz(),
            governor: governor(),
            ram_used_mb: ram_total.saturating_sub(ram_free) / 1024,
            ram_total_mb: ram_total / 1024,
            ram_available_mb: ram_free / 1024,
            swap_used_mb: swap_total.saturating_sub(swap_free) / 1024,
            swap_total_mb: swap_total / 1024,
            cpu_temp: cpu_temp_from(&temps),
            cpu_watts: cpu_watts_from(&powers),
            temps,
            fans,
            voltages,
            powers,
            gpu: read_gpu(),
            net_rx_kbps: net_rx,
            net_tx_kbps: net_tx,
            disk_read_kbps: disk_read,
            disk_write_kbps: disk_write,
            fps: mangohud_fps(),
            processes,
        }
    }
}

/// Reads the machine as often as the settings say (once a second unless the
/// user chose otherwise) and hands every reading to the window. The thread
/// lives as long as the program does.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut sampler = Sampler::new();
        let mut was_watched = true;
        loop {
            let every = u64::from(crate::config::load().hud.refresh_ms);
            std::thread::sleep(Duration::from_millis(every));
            // Nobody to show it to: the window and the overlay are hidden. The
            // program lives in the tray for hours, many of them during a game,
            // and a reading walks /proc, the sensors and nvidia-smi, which also
            // wakes the dedicated graphics card of a laptop.
            let watched = ["main", "overlay"]
                .iter()
                .any(|label| app.get_webview_window(label).and_then(|window| window.is_visible().ok()).unwrap_or(false));
            if !watched {
                was_watched = false;
                continue;
            }
            if !was_watched {
                // The first reading after a pause would average all of it.
                sampler = Sampler::new();
                was_watched = true;
                continue;
            }
            let snapshot = sampler.read();
            if let Ok(mut slot) = LATEST.lock() {
                *slot = Some(snapshot.clone());
            }
            let _ = app.emit(TICK, &snapshot);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fps_come_from_the_last_line_under_the_names_of_the_columns() {
        let dir = crate::testing::scratch("sgp-monitor", "fps");
        let log = dir.join("game.csv");
        // MangoHud writes a line about the system before the names.
        let mut text = String::from("os,cpu,gpu,ram,kernel,driver\nDebian,Ryzen,RX,32GB,7.3.5,Mesa\nfps,frametime,cpu_load\n");
        for frame in 0..5000 {
            text.push_str(&format!("{}.0,16.6,40\n", 60 + frame % 3));
        }
        text.push_str("144.5,6.9,55\n");
        fs::write(&log, text).unwrap();
        assert_eq!(fps_in_log(&log), Some(144.5));
    }

    #[test]
    fn a_log_without_frames_gives_nothing() {
        let dir = crate::testing::scratch("sgp-monitor", "empty");
        let log = dir.join("game.csv");
        fs::write(&log, "fps,frametime\n").unwrap();
        assert_eq!(fps_in_log(&log), None);
        assert_eq!(fps_in_log(&dir.join("missing.csv")), None);
    }

    #[test]
    fn the_battery_is_the_one_of_the_system_not_the_one_of_a_mouse() {
        let dir = crate::testing::scratch("sgp-monitor", "battery");
        let supply = |name: &str, kind: &str, scope: Option<&str>, capacity: &str, status: &str| {
            let folder = dir.join(name);
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("type"), format!("{kind}\n")).unwrap();
            if let Some(scope) = scope {
                fs::write(folder.join("scope"), format!("{scope}\n")).unwrap();
            }
            fs::write(folder.join("capacity"), format!("{capacity}\n")).unwrap();
            fs::write(folder.join("status"), format!("{status}\n")).unwrap();
        };
        supply("AC", "Mains", None, "0", "Unknown");
        supply("hidpp_battery_0", "Battery", Some("Device"), "40", "Discharging");
        assert_eq!(battery_in(&dir), None);
        supply("BAT0", "Battery", None, "76", "Discharging");
        assert_eq!(battery_in(&dir), Some(Battery { percent: 76.0, plugged: false }));
        supply("BAT0", "Battery", None, "80", "Not charging");
        assert_eq!(battery_in(&dir), Some(Battery { percent: 80.0, plugged: true }));
    }
}
