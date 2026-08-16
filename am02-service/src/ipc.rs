//! am02-service IPC 接收端（语言/主题下发通道）。
//!
//! 监听 unix domain socket（默认 `/run/am02-service.sock`，可用环境变量
//! `AM02_SERVICE_SOCK` 覆盖），接收换行分隔的 JSON 命令，更新共享的
//! [`DisplayConfig`] / [`PowerState`]，并通过 channel 通知主循环「立即重推一帧」。
//!
//! 命令格式（与 am02-decky 后端保持一致）：
//!   {"op":"set_language","lang":0|1}   // 0=中文 1=英文
//!   {"op":"set_theme","theme":0|1}     // 0=黑底 1=白底
//!   {"op":"set_time_format","format":0|1} // 0=12 小时制 1=24 小时制（pack[0x84]）
//!   {"op":"set_power_mode","mode":0|1|2|3,"tdp_watts":45.0}
//!                                      // mode 用 TdpMode 索引，tdp_watts 为 STAPM 瓦数
//!   {"op":"set_location","city":"深圳"} // 设置天气地区（城市名）；city 为空串 = 关闭天气
//!   {"op":"search_city","query":"南山"} // 城市模糊搜索，回写 {"ok":true,"cities":[{name,province,city,lat,lon},..]}
//!
//! 本模块独立于串口：插件/服务都绝不在此触碰 `/dev/ttyS0`。

// 本机 Windows 仅做 `cargo check` 编译验证：Unix 专属的 serve/handle_client
// 被 cfg 门控排除，导致下面这些项在 Windows 上「看似未使用」。Linux 目标上
// 它们都会被用到，故仅在非 unix 平台压制死代码/未用导入警告。
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::protocol::{DisplayConfig, Language, PowerState, TdpMode, Theme, TimeFormat};
use crate::weather::WeatherState;

/// 默认 socket 路径（与 am02-decky 后端一致）。
const DEFAULT_SOCKET_PATH: &str = "/run/am02-service.sock";
/// 单条命令上限（JSON 极短，512B 足够）。
const MAX_CMD_LEN: usize = 512;

fn socket_path() -> String {
    std::env::var("AM02_SERVICE_SOCK").unwrap_or_else(|_| DEFAULT_SOCKET_PATH.to_string())
}

/// 一条 IPC 命令。`lang`/`theme`/`format`/`mode`/`tdp_watts`/`city` 用 Option 区分「缺字段」与「值为 0」。
#[derive(Debug, Deserialize)]
struct IpcCommand {
    op: String,
    #[serde(default)]
    lang: Option<u8>,
    #[serde(default)]
    theme: Option<u8>,
    #[serde(default)]
    format: Option<u8>,
    #[serde(default)]
    mode: Option<u32>,
    #[serde(default)]
    tdp_watts: Option<f32>,
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    query: Option<String>,
}

/// 启动 IPC 服务：绑定 socket 后循环 accept + 处理。此函数阻塞运行，
/// 由调用方放到独立线程中，避免阻塞主循环。
#[cfg(unix)]
pub fn serve(
    disp: &Arc<Mutex<DisplayConfig>>,
    power: &Arc<Mutex<PowerState>>,
    weather: &Arc<Mutex<WeatherState>>,
    weather_wake: Sender<()>,
    notify: Sender<()>,
) -> Result<()> {
    use std::os::unix::net::UnixListener;

    let path = socket_path();
    // 清理上次异常退出遗留的 socket 文件，避免 bind 报 Address already in use
    let _ = std::fs::remove_file(&path);

    let listener = UnixListener::bind(&path)
        .with_context(|| format!("绑定 unix socket {path} 失败"))?;
    tracing::info!("IPC 已监听 {path}");

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Err(e) = handle_client(s, disp, power, weather, &weather_wake, &notify) {
                    tracing::warn!("IPC 命令处理失败: {e:#}");
                }
            }
            Err(e) => {
                tracing::error!("IPC accept 失败: {e}");
                // 退避，避免 accept 错误风暴
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
    Ok(())
}

/// 非 Unix 平台（本机 Windows 仅做 `cargo check` 编译验证）：不提供 IPC。
#[cfg(not(unix))]
pub fn serve(
    _disp: &Arc<Mutex<DisplayConfig>>,
    _power: &Arc<Mutex<PowerState>>,
    _weather: &Arc<Mutex<WeatherState>>,
    _weather_wake: Sender<()>,
    _notify: Sender<()>,
) -> Result<()> {
    tracing::warn!("IPC 仅支持 Unix 平台；当前为编译检查占位，未监听 socket");
    Ok(())
}

/// 读取单个连接的命令（直到换行 / EOF / 缓冲区满），解析并应用。
#[cfg(unix)]
fn handle_client(
    mut s: std::os::unix::net::UnixStream,
    disp: &Arc<Mutex<DisplayConfig>>,
    power: &Arc<Mutex<PowerState>>,
    weather: &Arc<Mutex<WeatherState>>,
    weather_wake: &Sender<()>,
    notify: &Sender<()>,
) -> Result<()> {
    use std::io::Read;

    let mut buf = [0u8; MAX_CMD_LEN];
    let mut n = 0usize;
    loop {
        if n >= MAX_CMD_LEN {
            break;
        }
        let k = s.read(&mut buf[n..])?;
        if k == 0 {
            break; // 对端关闭
        }
        n += k;
        if buf[..n].contains(&b'\n') {
            break;
        }
    }
    if n == 0 {
        return Ok(());
    }

    // 只取第一条命令（到换行为止），忽略后续多余字节
    let end = buf[..n].iter().position(|&b| b == b'\n').unwrap_or(n);
    let payload = std::str::from_utf8(&buf[..end])
        .context("IPC 命令非 UTF-8")?
        .trim();
    if payload.is_empty() {
        return Ok(());
    }

    let cmd: IpcCommand = serde_json::from_str(payload).context("IPC JSON 解析失败")?;
    let reply = apply_command(&cmd, disp, power, weather, weather_wake, notify);
    // get_state 需要把当前完整状态回写给请求方（插件）；set_* 命令不回复。
    if let Some(r) = reply {
        use std::io::Write;
        let _ = s.write_all(r.as_bytes());
        let _ = s.write_all(b"\n");
    }
    Ok(())
}

/// 应用命令：更新共享 DisplayConfig / PowerState / WeatherState；有实际变更时通知主循环重推。
///
/// 返回 `Some(json)` 表示需要回写给请求方（`get_state`）；其余命令返回 `None`。
fn apply_command(
    cmd: &IpcCommand,
    disp: &Arc<Mutex<DisplayConfig>>,
    power: &Arc<Mutex<PowerState>>,
    weather: &Arc<Mutex<WeatherState>>,
    weather_wake: &Sender<()>,
    notify: &Sender<()>,
) -> Option<String> {
    let mut changed = false;

    match cmd.op.as_str() {
        // 反向同步：插件周期轮询此命令，拿到副屏触摸切档后的最新状态
        // （mode/tdp_watts/language/theme/time_format/location）。
        "get_state" => {
            let d = disp.lock().unwrap();
            let p = power.lock().unwrap();
            let w = weather.lock().unwrap();
            return Some(
                serde_json::json!({
                    "mode": p.mode as u32,
                    "tdp_watts": p.tdp_watts,
                    "language": d.language as u8,
                    "theme": d.theme as u8,
                    "time_format": d.time_format as u8,
                    "location": w.location.clone().unwrap_or_default(),
                })
                .to_string(),
            );
        }
        // 城市模糊搜索：回写备选列表（每条含 name/province/city/lat/lon）
        "search_city" => {
            let query = cmd.query.as_deref().unwrap_or("");
            let cities = crate::weather::search_cities(query);
            return Some(
                serde_json::json!({ "ok": true, "cities": cities }).to_string(),
            );
        }
        "set_language" => {
            let new_lang = match cmd.lang {
                Some(0) => Some(Language::Zh),
                Some(1) => Some(Language::En),
                Some(other) => {
                    tracing::warn!("非法语言值 {other}（期望 0|1），忽略");
                    None
                }
                None => {
                    tracing::warn!("set_language 缺少 lang 字段");
                    None
                }
            };
            if let Some(l) = new_lang {
                let mut d = disp.lock().unwrap();
                if d.language != l {
                    d.language = l;
                    changed = true;
                }
            }
        }
        "set_theme" => {
            let new_theme = match cmd.theme {
                Some(0) => Some(Theme::Black),
                Some(1) => Some(Theme::White),
                Some(other) => {
                    tracing::warn!("非法主题值 {other}（期望 0|1），忽略");
                    None
                }
                None => {
                    tracing::warn!("set_theme 缺少 theme 字段");
                    None
                }
            };
            if let Some(t) = new_theme {
                let mut d = disp.lock().unwrap();
                if d.theme != t {
                    d.theme = t;
                    changed = true;
                }
            }
        }
        "set_time_format" => {
            let new_fmt = match cmd.format {
                Some(0) => Some(TimeFormat::H12),
                Some(1) => Some(TimeFormat::H24),
                Some(other) => {
                    tracing::warn!("非法时间制式值 {other}（期望 0|1），忽略");
                    None
                }
                None => {
                    tracing::warn!("set_time_format 缺少 format 字段");
                    None
                }
            };
            if let Some(f) = new_fmt {
                let mut d = disp.lock().unwrap();
                if d.time_format != f {
                    d.time_format = f;
                    changed = true;
                    tracing::info!("IPC 更新时间制式: {:?}", f);
                }
            }
        }
        "set_power_mode" => match (cmd.mode, cmd.tdp_watts) {
            (Some(m), Some(w)) => match TdpMode::from_index(m) {
                Some(mode) => {
                    let mut p = power.lock().unwrap();
                    if p.mode != mode || p.tdp_watts != w {
                        p.mode = mode;
                        p.tdp_watts = w;
                        changed = true;
                        tracing::info!("IPC 更新功耗模式: {:?} tdp={}W", mode, w);
                    }
                }
                None => tracing::warn!("非法模式值 {m}（期望 0..3），忽略"),
            },
            (None, _) => tracing::warn!("set_power_mode 缺少 mode 字段"),
            (_, None) => tracing::warn!("set_power_mode 缺少 tdp_watts 字段"),
        },
        "set_location" => {
            // city 为空串/空白 = 关闭天气；非空 = 设置地区（城市名）
            let new_loc = cmd
                .city
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(str::to_string);
            let mut w = weather.lock().unwrap();
            if w.location != new_loc {
                w.location = new_loc.clone();
                crate::weather::save_location(&w.location); // 持久化，重启后 get_state 反向同步不再清空
                if new_loc.is_none() {
                    w.snapshot = crate::protocol::WeatherSnapshot::default();
                    tracing::info!("IPC 关闭天气显示");
                } else {
                    tracing::info!("IPC 设置天气地区: {}", new_loc.as_deref().unwrap_or(""));
                }
                changed = true;
                // 唤醒天气线程立即拉取（成功后天气线程会再 notify 一次回推）
                let _ = weather_wake.send(());
            }
        }
        other => tracing::warn!("未知 IPC 操作: {other}"),
    }

    if changed {
        tracing::info!("IPC 命令已应用: {}", cmd.op);
        // 主循环 try_recv 到该信号后立即回推一帧，无需等下一个推送周期
        let _ = notify.send(());
    }
    None
}
