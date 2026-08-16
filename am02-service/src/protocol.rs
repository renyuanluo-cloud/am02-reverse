//! AM02 副屏协议层：253B 帧编码 + 29B 响应解码 + CRC。
//!
//! 字段偏移严格对应设计文档 §6.2，CRC = crc32(pack[4:253]) 小端写 [0:4]。
//! 帧长 253、响应 29 为硬约束。

/// 主机 -> 副屏帧长（硬约束）
pub const FRAME_LEN: usize = 253;
/// 副屏 -> 主机响应帧长（硬约束）
pub const RESP_LEN: usize = 29;

/// 主机 -> 副屏命令字（pack[4]）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cmd {
    OnConnect = 1,
    OnNormalInfo = 2,
    Special = 13,
}

/// 副屏 -> 主机子命令（响应 [20:24]）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SubCmd {
    /// 设置操作（功耗/风扇模式切换）
    Set = 1,
    /// 复位
    Reset = 2,
    /// 计算操作
    Compute = 3,
}

/// TDP 四档模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TdpMode {
    AaaGame = 0,
    ClassicGame = 1,
    RetroGame = 2,
    PcOffice = 3,
}

impl TdpMode {
    /// pack[0xdd] 处的模式名字符串："Force " + 模式名
    pub fn as_force_str(&self) -> &'static str {
        match self {
            TdpMode::AaaGame => "Force AAA Game",
            TdpMode::ClassicGame => "Force Classic Game",
            TdpMode::RetroGame => "Force Retro Game",
            TdpMode::PcOffice => "Force PC Office",
        }
    }

    /// 档位索引 u32 -> TdpMode（0=AAA/1=Classic/2=Retro/3=Office）。
    /// 副屏触摸切档的 `sub_param` 与 decky 下发 `set_power_mode` 的 `mode`
    /// 共用同一映射，保证单一映射源。
    pub fn from_index(i: u32) -> Option<TdpMode> {
        match i {
            0 => Some(TdpMode::AaaGame),
            1 => Some(TdpMode::ClassicGame),
            2 => Some(TdpMode::RetroGame),
            3 => Some(TdpMode::PcOffice),
            _ => None,
        }
    }
}

/// 功耗状态（档位 + 当前 STAPM 瓦数），主循环与 IPC 线程共享。
/// `mode` 写 pack[0x61]，`tdp_watts` 写 pack[0x69]。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerState {
    pub mode: TdpMode,
    pub tdp_watts: f32,
}

/// 系统信息快照（collector 产出，P1 用固定值填充）
#[derive(Debug, Clone, Default)]
pub struct SystemSnapshot {
    pub cpu_usage: f32,  // %
    pub cpu_power: f32,  // W
    pub cpu_temp: i32,   // ℃
    pub cpu_freq: i32,   // MHz
    pub gpu_usage: f32,  // %
    pub gpu_power: f32,  // W
    pub gpu_temp: i32,   // ℃
    pub gpu_freq: i32,   // MHz
    pub ram_used: i32,   // GB
    pub ram_total: i32,  // GB
    pub disk_used: i32,  // GB（Bazzite 分区 /var = nvme0n1p7）
    pub disk_total: i32, // GB
    pub fan_rpm: i32,
    pub net_status: i32, // 网络状态（pack[0x55]，0=断网/1=wifi）
    pub net_down: i32, // KB/s
    pub net_up: i32,   // KB/s
    pub fps: i32,      // 游戏内 FPS，桌面=0
    pub tdp_watts: f32, // W
}

/// 天气快照（weather 模块产出，主循环读后写 pack 字符串字段）。
///
/// 字段偏移来自逆向（AYASpaceCef.exe `CMiniPCLauncher::UpdateInfo` + 副屏
/// `hour_weather` 组件）：
///   0x85 城市名 / 0x95 天气状况 / 0xa5 省份 / 0xc5 气温 / 0xd1 风向 / 0xdd 风力。
/// 其中 0xc5（气温，格式 `X.X℃`）与 0xdd（风力，格式 `N级`）已实锤；
/// 0x85/0x95/0xa5/0xd1 的语义按字段长度 + 副屏显示顺序推断，见 weather.rs。
#[derive(Debug, Clone, Default)]
pub struct WeatherSnapshot {
    /// pack[0x85] 城市名
    pub city: String,
    /// pack[0x95] 天气状况（如「晴」「多云」）
    pub weather: String,
    /// pack[0xa5] 省份/区域（OpenWeatherMap 不返回，预留空）
    pub province: String,
    /// 气温（℃），写 pack[0xc5]，格式化为 `X.X℃`
    pub temperature_c: f32,
    /// pack[0xd1] 风向（如「东北风」）
    pub wind_direction: String,
    /// 风力等级（蒲福风级），写 pack[0xdd]，格式化为 `N级`
    pub wind_power: u8,
}

/// 语言（pack[0x79]）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Language {
    Zh = 0,
    En = 1,
}

/// 主题（pack[0x7a]）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Theme {
    Black = 0,
    White = 1,
}

/// 12/24 小时制（pack[0x84]）。
///
/// 副屏固件 `clock_page2` 的 data_init 读该字节：
///   `cmp r3, #0; beq 12h分支` —— 0 走 AM/PM 12h 显示（hour>12 减 12），
///   非 0 走 24h 直接显示。故 **0=12 小时制、1=24 小时制**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TimeFormat {
    H12 = 0,
    H24 = 1,
}

/// 显示配置（语言/主题/时间制式）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayConfig {
    pub language: Language,
    pub theme: Theme,
    pub time_format: TimeFormat,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            language: Language::Zh,
            theme: Theme::Black,
            // 副屏时钟页默认 24 小时制（与 pack[0x80] 每帧推的 24h 小时值一致）
            time_format: TimeFormat::H24,
        }
    }
}

/// 帧编码器（253 字节）
pub struct FrameEncoder {
    buf: [u8; FRAME_LEN],
}

impl FrameEncoder {
    pub fn new() -> Self {
        Self { buf: [0u8; FRAME_LEN] }
    }

    /// 构建 OnConnect 帧
    pub fn on_connect(&mut self) -> &[u8; FRAME_LEN] {
        self.buf.fill(0);
        self.buf[4] = Cmd::OnConnect as u8;
        self.finalize();
        &self.buf
    }

    /// 构建 OnNormalInfo 帧（填 share_data 字段 + CRC）
    pub fn on_normal_info(
        &mut self,
        snap: &SystemSnapshot,
        disp: &DisplayConfig,
        mode: TdpMode,
        weather: &WeatherSnapshot,
    ) -> &[u8; FRAME_LEN] {
        self.buf.fill(0);
        self.buf[4] = Cmd::OnNormalInfo as u8;
        self.buf[13] = 0; // uex_cmd
        self.buf[14] = 0; // uex_param

        // 数值字段（小端，偏移严格按 §6.2）
        put_f32(&mut self.buf, 0x21, snap.cpu_usage);
        put_f32(&mut self.buf, 0x25, snap.cpu_power);
        put_i32(&mut self.buf, 0x29, snap.cpu_temp);
        put_i32(&mut self.buf, 0x2d, snap.cpu_freq);
        put_f32(&mut self.buf, 0x31, snap.gpu_usage);
        put_f32(&mut self.buf, 0x35, snap.gpu_power);
        put_i32(&mut self.buf, 0x39, snap.gpu_temp);
        put_i32(&mut self.buf, 0x3d, snap.gpu_freq);
        put_i32(&mut self.buf, 0x41, snap.ram_used);
        put_i32(&mut self.buf, 0x45, snap.ram_total);
        put_i32(&mut self.buf, 0x49, snap.disk_used);
        put_i32(&mut self.buf, 0x4d, snap.disk_total);
        put_i32(&mut self.buf, 0x51, snap.fan_rpm);
        put_i32(&mut self.buf, 0x55, snap.net_status); // 网络状态（反汇编确认，非电池）
        put_i32(&mut self.buf, 0x59, snap.net_down);
        put_i32(&mut self.buf, 0x5d, snap.net_up);
        put_u32(&mut self.buf, 0x61, mode as u32); // 档位索引 PowerMode（0=AAA/1=Classic/2=Retro/3=Office），副屏据此勾选档位
        put_i32(&mut self.buf, 0x65, 0); // pack[0x65] 未读，保持 0
        put_f32(&mut self.buf, 0x69, snap.tdp_watts);
        put_f32(&mut self.buf, 0x6d, snap.fps as f32); // FPS（反汇编确认 pack[0x6d]=fps，f2iz→r4+0x0）

        // 语言/主题
        self.buf[0x79] = disp.language as u8;
        self.buf[0x7a] = disp.theme as u8;

        // 时间（本地时间，每帧推）。逆向结论：原版 AYASpaceCef.exe 在
        // SendNormalInfo 里调 GetLocalTime() 填 SYSTEMTIME，再逐字段写：
        //   pack[0x7b:0x7d] = wYear(u16 LE)
        //   pack[0x7d]      = wMonth
        //   pack[0x7e]      = wDay
        //   pack[0x7f]      = wDayOfWeek(0=周日)
        //   pack[0x80]      = wHour(0-23)  ← 恒 24h，12/24 由 0x84 标志位控制显示
        //   pack[0x81]      = wMinute
        //   pack[0x82]      = wSecond
        write_local_time(&mut self.buf);

        // 时钟页显示开关 + 12/24 制标志位（原版 SendNormalInfo 每帧从
        // this->0x2f1/this->0x2f0 写入 pack[0x83]/pack[0x84]）。
        //   0x83 = 时钟页显示开关（0=隐藏/跳转，非0=显示时钟页）
        //   0x84 = 12/24 制标志位（0=12h，1=24h）
        self.buf[0x83] = 1;
        self.buf[0x84] = disp.time_format as u8;

        // 天气字符串字段（逆向标定，见 WeatherSnapshot 注释）。字符串按定长
        // 缓冲拷贝（buf 已 fill(0)，剩余字节保持 0，副屏读到 0 即视为结束）。
        put_str(&mut self.buf, 0x85, 16, &weather.city);
        put_str(&mut self.buf, 0x95, 16, &weather.weather);
        put_str(&mut self.buf, 0xa5, 32, &weather.province);
        if !weather.city.is_empty() || weather.temperature_c != 0.0 {
            // 气温：逆向确认格式 `X.X℃`（原版按 this+0x2f2 切 ℃/℉，这里固定 ℃）
            let t = format!("{:.1}℃", weather.temperature_c);
            put_str(&mut self.buf, 0xc5, 12, &t);
        }
        put_str(&mut self.buf, 0xd1, 12, &weather.wind_direction);
        if weather.wind_power > 0 {
            // 风力：逆向确认格式 `N级`（蒲福风级）
            let w = format!("{}级", weather.wind_power);
            put_str(&mut self.buf, 0xdd, 20, &w);
        }

        // 升级字段（正常=0）
        put_u16(&mut self.buf, 0x1b, 0);
        put_u32(&mut self.buf, 0x1d, 0);

        self.finalize();
        &self.buf
    }

    /// CRC 计算：crc32(pack[4:253])，小端写 [0:4]
    fn finalize(&mut self) {
        let crc = crc32fast::hash(&self.buf[4..FRAME_LEN]);
        self.buf[0..4].copy_from_slice(&crc.to_le_bytes());
    }
}

/// 副屏 -> 主机响应解析结果（29 字节）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Response {
    pub recv_crc32: u32, // [0:4]  回显 CRC
    pub cmd: u8,         // [4]    命令回显
    pub sub_cmd: u32,    // [20:24] 副屏子命令
    pub sub_param: u32,  // [24:28] 子命令参数
    pub status: u8,      // [28]   状态字节
}

impl Response {
    pub fn parse(buf: &[u8; RESP_LEN]) -> Self {
        Self {
            recv_crc32: u32::from_le_bytes(buf[0..4].try_into().unwrap()),
            cmd: buf[4],
            sub_cmd: u32::from_le_bytes(buf[20..24].try_into().unwrap()),
            sub_param: u32::from_le_bytes(buf[24..28].try_into().unwrap()),
            status: buf[28],
        }
    }

    /// 握手判据：回显 CRC == 发送 CRC，且命令为 OnConnect
    pub fn is_valid_handshake(&self, sent_crc: u32) -> bool {
        self.recv_crc32 == sent_crc && self.cmd == Cmd::OnConnect as u8
    }
}

// ── 辅助：小端写入 ──

fn put_f32(buf: &mut [u8], off: usize, v: f32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_i32(buf: &mut [u8], off: usize, v: i32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_u16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// 定长字符串拷贝：最多写入 `max_len` 字节，剩余保持 0（buf 已 fill(0)）。
fn put_str(buf: &mut [u8], off: usize, max_len: usize, s: &str) {
    let b = s.as_bytes();
    let n = b.len().min(max_len);
    buf[off..off + n].copy_from_slice(&b[..n]);
}

/// 写当前本地时间（北京时区）到 pack 时间字段。
///
/// 用 `libc::localtime_r`（等价原版 `GetLocalTime`，非 UTC），取系统
/// `/etc/localtime`/`TZ` 决定的本地时间。字段布局与 SYSTEMTIME 一一对应：
///   year=wYear, month=wMonth, day=wDay, wday=wDayOfWeek(0=周日),
///   hour=wHour(0-23), min=wMinute, sec=wSecond。
///
/// 本机 Windows 仅做 `cargo check` 编译验证：`localtime_r` 是 Unix 专属，
/// Windows 上无此符号，故非 unix 平台退化为空操作（字段保持 0）。
#[cfg(unix)]
fn write_local_time(buf: &mut [u8]) {
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut t: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut t).is_null() {
            return; // 失败保持 0，不 panic（副屏读到 0 视为无时间，下帧重试）
        }
        put_u16(buf, 0x7b, (t.tm_year + 1900) as u16);
        buf[0x7d] = (t.tm_mon + 1) as u8;
        buf[0x7e] = t.tm_mday as u8;
        buf[0x7f] = t.tm_wday as u8;
        buf[0x80] = t.tm_hour as u8;
        buf[0x81] = t.tm_min as u8;
        buf[0x82] = t.tm_sec as u8;
    }
}

/// Windows 编译检查占位：时间字段保持 0（buf 已 fill(0)），不做任何事。
#[cfg(not(unix))]
fn write_local_time(_buf: &mut [u8]) {}

