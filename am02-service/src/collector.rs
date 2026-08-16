//! P2：sysfs 采集器。
//!
//! 每秒读 /sys、/proc，产出 [`SystemSnapshot`]。所有读取失败回退默认值（0），不 panic。
//! 速率类字段（CPU 使用率/功耗、网速）基于两次采样差值，首帧为 0。

use std::time::Instant;

use crate::protocol::SystemSnapshot;

/// 采集器：持有上次采样基线，用于差值计算。
pub struct Collector {
    // CPU 使用率基线（/proc/stat）
    last_cpu: Option<CpuTimes>,
    // RAPL 能量基线（/sys/class/powercap/*/energy_uj）
    last_rapl_energy_uj: Option<u64>,
    last_rapl_at: Option<Instant>,
    // 网速基线（rx_bytes/tx_bytes）
    net_iface: Option<String>,
    last_net_rx: Option<u64>,
    last_net_tx: Option<u64>,
    last_net_at: Option<Instant>,
}

#[derive(Clone, Copy)]
struct CpuTimes {
    idle: u64,
    total: u64,
}

impl Collector {
    /// 新建并预采一帧基线，使首次 `collect` 即有有效速率值。
    pub fn new() -> Self {
        let mut c = Self {
            last_cpu: None,
            last_rapl_energy_uj: None,
            last_rapl_at: None,
            net_iface: None,
            last_net_rx: None,
            last_net_tx: None,
            last_net_at: None,
        };
        let _ = c.collect(); // 预热：仅建立基线
        c
    }

    /// 采集一帧系统信息快照。
    pub fn collect(&mut self) -> SystemSnapshot {
        let gpu = read_gpu_metrics();
        let (ram_used, ram_total) = ram_usage_mb();
        let (disk_used, disk_total) = disk_usage_mb();
        let (net_down, net_up) = self.net_speed_kbps();

        SystemSnapshot {
            cpu_usage: self.cpu_usage(),
            cpu_power: self.cpu_power(),
            cpu_temp: cpu_temp_c(),
            cpu_freq: cpu_freq_mhz(),
            gpu_usage: gpu.usage,
            gpu_power: gpu.power,
            gpu_temp: gpu.temp,
            gpu_freq: gpu.freq,
            ram_used,
            ram_total,
            disk_used,
            disk_total,
            fan_rpm: read_fan_rpm(), // IT8620E Super-IO 风扇转速（it87 驱动，force_id=0x8620）
            net_status: net_status(), // 网络状态（eno1 operstate）
            net_down,
            net_up,
            fps: crate::fps_adapter::read_fps(), // P4：gamescope 消息队列
            tdp_watts: 0.0,  // P3 接入 ryzenadj
        }
    }

    /// CPU 使用率：/proc/stat 两次采样差值。
    fn cpu_usage(&mut self) -> f32 {
        let Some(now) = read_cpu_times() else {
            self.last_cpu = None;
            return 0.0;
        };
        let usage = match self.last_cpu {
            Some(prev) => {
                let dt = now.total.saturating_sub(prev.total);
                let di = now.idle.saturating_sub(prev.idle);
                if dt == 0 {
                    0.0
                } else {
                    ((dt - di) as f32 / dt as f32 * 100.0).clamp(0.0, 100.0)
                }
            }
            None => 0.0,
        };
        self.last_cpu = Some(now);
        usage
    }

    /// CPU 功耗：RAPL energy_uj 差值 / 时间（µJ → W）。
    fn cpu_power(&mut self) -> f32 {
        let Some((energy, max_range)) = read_rapl_package() else {
            self.last_rapl_energy_uj = None;
            self.last_rapl_at = None;
            return 0.0;
        };
        let now = Instant::now();
        let mut watts = 0.0_f64;
        if let (Some(last), Some(last_at)) = (self.last_rapl_energy_uj, self.last_rapl_at) {
            let dt = now.duration_since(last_at).as_secs_f64();
            if dt > 0.0 {
                let de = if energy >= last {
                    energy - last
                } else {
                    energy + max_range - last // 计数器回绕
                };
                watts = de as f64 / dt / 1_000_000.0;
            }
        }
        self.last_rapl_energy_uj = Some(energy);
        self.last_rapl_at = Some(now);
        watts as f32
    }

    /// 网速：rx_bytes/tx_bytes 差值 / 时间，单位 KB/s（下行, 上行）。
    fn net_speed_kbps(&mut self) -> (i32, i32) {
        if self.net_iface.is_none() {
            self.net_iface = discover_net_iface();
        }
        let Some(iface) = self.net_iface.clone() else {
            return (0, 0);
        };
        let rx = read_u64_file(&format!("/sys/class/net/{iface}/statistics/rx_bytes"));
        let tx = read_u64_file(&format!("/sys/class/net/{iface}/statistics/tx_bytes"));
        let (Some(rx), Some(tx)) = (rx, tx) else {
            return (0, 0);
        };

        let now = Instant::now();
        let mut down = 0i32;
        let mut up = 0i32;
        if let (Some(lrx), Some(ltx), Some(last_at)) =
            (self.last_net_rx, self.last_net_tx, self.last_net_at)
        {
            let dt = now.duration_since(last_at).as_secs_f64();
            if dt > 0.0 {
                down = (rx.saturating_sub(lrx) as f64 / dt / 1024.0) as i32;
                up = (tx.saturating_sub(ltx) as f64 / dt / 1024.0) as i32;
            }
        }
        self.last_net_rx = Some(rx);
        self.last_net_tx = Some(tx);
        self.last_net_at = Some(now);
        (down, up)
    }
}

// ── 通用读取辅助 ──

fn read_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn read_u64_file(path: &str) -> Option<u64> {
    read_file(path)?.trim().parse().ok()
}

// ── CPU 使用率（/proc/stat）──

fn read_cpu_times() -> Option<CpuTimes> {
    let s = read_file("/proc/stat")?;
    let line = s.lines().next()?;
    let mut it = line.split_whitespace();
    if it.next() != Some("cpu") {
        return None;
    }
    let vals: Vec<u64> = it.filter_map(|p| p.parse().ok()).collect();
    if vals.len() < 4 {
        return None;
    }
    let idle = vals[3] + vals.get(4).copied().unwrap_or(0); // idle + iowait
    let total: u64 = vals.iter().sum();
    Some(CpuTimes { idle, total })
}

// ── CPU 频率（scaling_cur_freq，kHz → MHz 平均）──

fn cpu_freq_mhz() -> i32 {
    let mut sum = 0u64;
    let mut count = 0u64;
    if let Ok(entries) = std::fs::read_dir("/sys/devices/system/cpu") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(rest) = name.strip_prefix("cpu") else {
                continue;
            };
            if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            if let Some(khz) = read_u64_file(&format!(
                "/sys/devices/system/cpu/{name}/cpufreq/scaling_cur_freq"
            )) {
                sum += khz;
                count += 1;
            }
        }
    }
    if count == 0 {
        0
    } else {
        (sum / count / 1000) as i32
    }
}

// ── CPU 温度（k10temp，m°C → °C）──

fn cpu_temp_c() -> i32 {
    // 实测 k10temp 挂在 hwmon3
    if let Some(v) = read_u64_file("/sys/class/hwmon/hwmon3/temp1_input") {
        return (v / 1000) as i32;
    }
    // 回退：按 name 扫描 k10temp（hwmon 索引跨重启不稳定）
    if let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") {
        for e in entries.flatten() {
            let base = e.path();
            if let Some(name) = read_file(&format!("{}/name", base.display())) {
                if name.trim() == "k10temp" {
                    if let Some(v) = read_u64_file(&format!("{}/temp1_input", base.display())) {
                        return (v / 1000) as i32;
                    }
                }
            }
        }
    }
    0
}

// ── CPU 功耗（RAPL package energy_uj）──

fn read_rapl_package() -> Option<(u64, u64)> {
    let entries = std::fs::read_dir("/sys/class/powercap").ok()?;
    let mut fallback: Option<std::path::PathBuf> = None;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("intel-rapl") {
            continue;
        }
        let base = e.path();
        // package 域：intel-rapl:0（或老内核的 intel-rapl），优先
        if name == "intel-rapl:0" || name == "intel-rapl" {
            if let Some(energy) = read_u64_file(&format!("{}/energy_uj", base.display())) {
                let max = read_u64_file(&format!("{}/max_energy_range_uj", base.display()))
                    .unwrap_or(0);
                return Some((energy, max));
            }
        } else if fallback.is_none() {
            fallback = Some(base);
        }
    }
    let base = fallback?;
    let energy = read_u64_file(&format!("{}/energy_uj", base.display()))?;
    let max = read_u64_file(&format!("{}/max_energy_range_uj", base.display())).unwrap_or(0);
    Some((energy, max))
}

// ── RAM（/proc/meminfo，KB → GB，副屏单位是 GB）──

fn ram_usage_mb() -> (i32, i32) {
    let s = read_file("/proc/meminfo").unwrap_or_default();
    let mut total_kb = 0u64;
    let mut avail_kb = 0u64;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total_kb = first_num(rest);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            avail_kb = first_num(rest);
        }
    }
    let total = (total_kb / 1024 / 1024) as i32;
    let used = (total_kb.saturating_sub(avail_kb) / 1024 / 1024) as i32;
    (used, total)
}

fn first_num(s: &str) -> u64 {
    s.split_whitespace()
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

// ── 磁盘（对齐 Windows：枚举所有已挂载数据分区加总，Linux 用 libc）──

#[cfg(target_os = "linux")]
fn disk_usage_mb() -> (i32, i32) {
    use std::collections::HashSet;
    use std::ffi::CString;

    const GB: u64 = 1024 * 1024 * 1024;
    // 对齐 Windows 语义：GetLogicalDriveStringsW 枚举所有盘符逐个加总。
    // 这里枚举 /proc/mounts 里所有已挂载的块设备分区，去重后加总容量。
    // 忽略 ntfs（Windows 分区不归 Bazzite 管）、vfat(EFI)、ext4(boot)、swap。
    // btrfs 有多个子卷挂载点（/var /home /etc...），按块设备名去重只算一次。
    let mounts = read_file("/proc/mounts").unwrap_or_default();
    let mut seen: HashSet<String> = HashSet::new();
    let mut total_gb: u64 = 0;
    let mut used_gb: u64 = 0;

    for line in mounts.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 3 {
            continue;
        }
        let (dev, mnt, fstype) = (f[0], f[1], f[2]);
        // 只处理块设备分区（nvme/sd/mmcblk/vd），跳过 loop/dm/zram 等虚拟设备
        let base = dev.rsplit('/').next().unwrap_or(dev);
        let is_blk = base.starts_with("nvme")
            || base.starts_with("sd")
            || base.starts_with("mmcblk")
            || base.starts_with("vd");
        if !is_blk {
            continue;
        }
        // 排除非数据分区
        match fstype {
            "ntfs" | "vfat" | "swap" | "ext4" | "ext3" | "ext2" | "xfs" | "squashfs"
            | "overlay" | "tmpfs" => continue,
            _ => {}
        }
        // 按块设备去重（btrfs 子卷只算一次）
        if !seen.insert(dev.to_string()) {
            continue;
        }
        // statvfs 拿该分区容量（f_blocks * f_frsize）
        let Ok(path) = CString::new(mnt) else {
            continue;
        };
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(path.as_ptr(), &mut st) } != 0 {
            continue;
        }
        let frsize = st.f_frsize as u64;
        let total = (st.f_blocks as u64).saturating_mul(frsize);
        let used = (st.f_blocks as u64)
            .saturating_sub(st.f_bfree as u64)
            .saturating_mul(frsize);
        // 副屏单位是 GB，四舍五入取整（加 0.5GB 再除）
        total_gb = total_gb.saturating_add(total.saturating_add(GB / 2) / GB);
        used_gb = used_gb.saturating_add(used.saturating_add(GB / 2) / GB);
    }

    (used_gb as i32, total_gb as i32)
}

#[cfg(not(target_os = "linux"))]
fn disk_usage_mb() -> (i32, i32) {
    (0, 0)
}

// ── GPU（amdgpu gpu_metrics 二进制）──

#[derive(Default)]
struct GpuMetrics {
    usage: f32,
    temp: i32,
    power: f32,
    freq: i32,
}

fn read_gpu_metrics() -> GpuMetrics {
    // 实测 iGPU 在 card1（card0 非目标设备）
    for card in ["card1", "card0", "card2", "card3"] {
        let path = format!("/sys/class/drm/{card}/device/gpu_metrics");
        if let Ok(buf) = std::fs::read(&path) {
            if let Some(m) = parse_gpu_metrics(&buf) {
                return m;
            }
        }
    }
    GpuMetrics::default()
}

fn u16_at(buf: &[u8], off: usize) -> u16 {
    if off + 2 <= buf.len() {
        u16::from_le_bytes([buf[off], buf[off + 1]])
    } else {
        0
    }
}

/// 解析 amdgpu gpu_metrics 二进制，按 common_header 的 format_revision 选布局。
/// Phoenix（Radeon 780M / SMU13）实际是 v2_1（format_revision == 2）。
fn parse_gpu_metrics(buf: &[u8]) -> Option<GpuMetrics> {
    if buf.len() < 4 {
        return None;
    }
    // common_header: structure_size(u16) + format_revision(u8) + content_revision(u8)
    let format_rev = buf[2];

    match format_rev {
        // v1.x（旧内核）
        1 => {
            let usage = u16_at(buf, 16) as f32 / 100.0; // average_gfx_activity 是 centipercent(0-10000)，÷100 才是 %
            let temp = (u16_at(buf, 6) as i32) / 100; // temperature_hotspot, 0.01℃ → ℃
            let power = u16_at(buf, 22) as f32 / 1000.0; // average_socket_power mW → W
            let freq = u16_at(buf, 54) as i32; // current_gfxclk MHz
            Some(GpuMetrics { usage, temp, power, freq })
        }
        // v2.x（SMU13/Phoenix 用 v2_1）
        2 => {
            // 注意 system_clock_counter 是 u64（offset 32~39），后续字段在其后
            let temp = (u16_at(buf, 4) as i32) / 100; // temperature_gfx, 0.01℃ → ℃
            let usage = u16_at(buf, 28) as f32 / 100.0; // average_gfx_activity 是 centipercent(0-10000)，÷100 才是 %
            let power = u16_at(buf, 46) as f32 / 1000.0; // average_gfx_power mW → W
            let freq = u16_at(buf, 64) as i32; // average_gfxclk_frequency MHz（current_gfxclk 空闲时=65535 无效）
            Some(GpuMetrics { usage, temp, power, freq })
        }
        _ => None,
    }
}

// ── 网卡发现（优先物理、up 的接口，跳过 lo）──

fn discover_net_iface() -> Option<String> {
    let entries = std::fs::read_dir("/sys/class/net").ok()?;
    let mut best: Option<(u8, String)> = None;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name == "lo" {
            continue;
        }
        let base = e.path();
        let physical = base.join("device").exists();
        let up = read_file(&format!("{}/operstate", base.display()))
            .map(|s| s.trim() == "up")
            .unwrap_or(false);
        let prio = match (physical, up) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        };
        if best.as_ref().map(|(p, _)| prio < *p).unwrap_or(true) {
            best = Some((prio, name));
        }
    }
    best.map(|(_, n)| n)
}

// ── 网络状态（pack[0x55]，0=断网 / 1=wifi）──

fn net_status() -> i32 {
    let iface = discover_net_iface().unwrap_or_else(|| "eno1".to_string());
    let oper = read_file(&format!("/sys/class/net/{iface}/operstate"))
        .map(|s| s.trim() == "up")
        .unwrap_or(false);
    if oper { 1 } else { 0 }
}

// ── 风扇转速（pack[0x51]，IT8620E Super-IO 经 it87 驱动暴露）──

fn read_fan_rpm() -> i32 {
    // AM02 风扇转速由 ITE IT8620E Super-IO 芯片提供，Linux 用 it87 驱动
    // （force_id=0x8620，因 DSDT 把端口声明成 PNP0C02 资源需强制）。驱动加载后
    // 在 hwmon 暴露 fan*_input（本机实测 fan2=CPU 风扇，fan1 可能不存在）。
    let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") else {
        return 0;
    };
    for e in entries.flatten() {
        let base = e.path();
        let name = read_file(&format!("{}/name", base.display())).unwrap_or_default();
        let name = name.trim();
        if !(name.starts_with("it87") || name.starts_with("it8620") || name.starts_with("it8")) {
            continue;
        }
        // 返回第一个非零的风扇转速（fan1..fan6）
        for i in 1..=6 {
            if let Some(v) = read_u64_file(&format!("{}/fan{}_input", base.display(), i)) {
                if v > 0 {
                    return v as i32;
                }
            }
        }
    }
    0
}
