//! AM02 副屏复刻服务：串口 + DTR/RTS + 握手 + 采集推送 + 副屏命令响应。
//!
//! 串口 /dev/ttyS0，115200 8N1 raw，无流控。同步主循环（P3 从简，不用 tokio 并发）。

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serialport::{ClearBuffer, DataBits, FlowControl, Parity, SerialPort, StopBits};

mod collector;
mod fps_adapter;
mod ipc;
mod power_adapter;
mod protocol;
mod state;
mod weather;

use collector::Collector;
use protocol::{
    DisplayConfig, FrameEncoder, Language, PowerState, Response, TdpMode, Theme, TimeFormat,
    WeatherSnapshot, RESP_LEN,
};
use weather::{spawn_weather_loop, WeatherState};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let mut port: Box<dyn SerialPort> = open_port()?;
    tracing::info!("串口已打开，DTR/RTS 已使能");

    // 状态机 Idle -> Connecting -> Connected（握手）
    handshake(&mut *port)?;

    // 状态机 Connected -> Running（主循环）
    let mut col = Collector::new();
    // 显示配置由主循环与 IPC 线程共享：IPC 线程写、主循环读
    let disp_shared = Arc::new(Mutex::new(DisplayConfig {
        language: Language::Zh,
        theme: Theme::Black,
        time_format: TimeFormat::H24,
    }));
    let mut enc = FrameEncoder::new();
    // 功耗状态由主循环与 IPC 线程共享：副屏触摸切档（handle_command）与
    // decky 下发（set_power_mode）都会写这里，主循环读后推 pack[0x61]/[0x69]。
    let power_shared = Arc::new(Mutex::new(PowerState {
        mode: TdpMode::AaaGame,
        tdp_watts: 45.0,
    }));
    let mut frame_count: u64 = 0;

    // IPC 接收线程（语言/主题/功耗/地区下发），独立于串口主循环
    let (ipc_tx, ipc_rx) = std::sync::mpsc::channel::<()>();
    // 天气后台线程：周期拉取 + IPC 设地区后立即唤醒；失败保持旧值不阻塞主循环
    let weather_shared = Arc::new(Mutex::new(WeatherState::new()));
    let weather_wake = spawn_weather_loop(Arc::clone(&weather_shared), ipc_tx.clone());
    {
        let disp_clone = Arc::clone(&disp_shared);
        let power_clone = Arc::clone(&power_shared);
        let weather_clone = Arc::clone(&weather_shared);
        std::thread::spawn(move || {
            if let Err(e) = ipc::serve(&disp_clone, &power_clone, &weather_clone, weather_wake, ipc_tx) {
                tracing::error!("IPC 服务线程退出: {e:#}");
            }
        });
    }

    // 调试：打印一次初始采集值，核对单位
    let dbg_power = *power_shared.lock().unwrap();
    let mut dbg = col.collect();
    dbg.tdp_watts = dbg_power.tdp_watts; // 调试日志/帧显示真实推的 TDP（缓存值），避免 0W 误导
    tracing::info!(
        "初始采集: cpu_usage={:.1}% cpu_power={:.1}W cpu_temp={} cpu_freq={}MHz gpu_usage={:.1}% gpu_power={:.1}W gpu_temp={} gpu_freq={}MHz ram={}/{}GB disk={}/{}GB fan={}rpm net={}/{}KB/s tdp={}W fps={}",
        dbg.cpu_usage, dbg.cpu_power, dbg.cpu_temp, dbg.cpu_freq,
        dbg.gpu_usage, dbg.gpu_power, dbg.gpu_temp, dbg.gpu_freq,
        dbg.ram_used, dbg.ram_total, dbg.disk_used, dbg.disk_total, dbg.fan_rpm,
        dbg.net_down, dbg.net_up, dbg.tdp_watts, dbg.fps
    );
    let disp = *disp_shared.lock().unwrap();
    let weather_dbg = weather_shared.lock().unwrap().snapshot.clone();
    let frame_dbg = enc.on_normal_info(&dbg, &disp, dbg_power.mode, &weather_dbg);
    tracing::info!("帧字节: 0x21..0x41={:02x?} 0x41..0x61={:02x?} 0x61..0x79={:02x?}", &frame_dbg[0x21..0x41], &frame_dbg[0x41..0x61], &frame_dbg[0x61..0x79]);
    tracing::info!(
        "时间字段: 0x79={:02x} 0x7a={:02x} 年={} 月={} 日={} 周={} 时={} 分={} 秒={} 0x83(时钟页)={:02x} 0x84(12/24)={:02x}",
        frame_dbg[0x79],
        frame_dbg[0x7a],
        u16::from_le_bytes([frame_dbg[0x7b], frame_dbg[0x7c]]),
        frame_dbg[0x7d],
        frame_dbg[0x7e],
        frame_dbg[0x7f],
        frame_dbg[0x80],
        frame_dbg[0x81],
        frame_dbg[0x82],
        frame_dbg[0x83],
        frame_dbg[0x84],
    );

    loop {
        // 1. 先处理输入（副屏切档 + IPC 下发），再推帧。这样推出的那一帧
        //    一定携带最新 pack[0x61]/pack[0x79]/…，副屏不会先收到「旧档位帧」
        //    而弹回（回弹根治的关键：处理顺序从「先推后读」改为「先读后推」）。
        let mut state_changed = false;

        // 1a. 读副屏命令（短超时：29B 响应 ~3ms 到齐，15ms 足够读满；无响应时
        //     只多花 ~15ms，不再拖慢推帧周期）
        if let Some(resp_buf) = read_response(&mut *port, 15) {
            let resp = Response::parse(&resp_buf);
            if handle_command(&resp, &power_shared) {
                state_changed = true;
            }
        }

        // 1b. 处理 IPC 语言/主题/功耗/天气变更
        if ipc_rx.try_recv().is_ok() {
            state_changed = true;
        }

        // 2. 采集真实数据并推一帧（TDP 值用缓存，避免每秒 fork ryzenadj；
        //    pack[0x61]/pack[0x69] 已在步骤 1 更新到最新）
        let disp = *disp_shared.lock().unwrap();
        let power = *power_shared.lock().unwrap();
        let mut snap = col.collect();
        snap.tdp_watts = power.tdp_watts;
        let weather = weather_shared.lock().unwrap().snapshot.clone();
        let frame = enc.on_normal_info(&snap, &disp, power.mode, &weather);

        if let Err(e) = port.write_all(frame) {
            tracing::error!("写串口失败: {e:#}，进入重连");
            match reconnect() {
                Ok(p) => {
                    port = p;
                    tracing::info!("重连成功");
                }
                Err(e) => {
                    tracing::error!("重连失败: {e:#}，5 秒后重试");
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
            continue;
        }
        port.flush().ok();

        if state_changed {
            tracing::info!("已回推新状态 {:?} {:?}", disp, power);
        }

        // 周期网速日志（每 25 帧），便于观测 pack[0x59]/[0x5d] 实际推的值
        frame_count += 1;
        if frame_count % 25 == 0 {
            tracing::info!("网速: down={} up={} KB/s", snap.net_down, snap.net_up);
        }

        // 3. 推送周期：副屏确认窗口 ~200ms，周期须显著小于该窗口，确保
        //    副屏触摸/IPC 切档后回推帧能在窗口内到达（不弹回）。
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// 打开串口并使能 DTR/RTS（副屏通信前置条件）。
fn open_port() -> Result<Box<dyn SerialPort>> {
    let mut port = serialport::new("/dev/ttyS0", 115_200)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .timeout(Duration::from_millis(100))
        .open()
        .context("打开串口 /dev/ttyS0 失败")?;
    port.write_data_terminal_ready(true)
        .context("设置 DTR 失败")?;
    port.write_request_to_send(true)
        .context("设置 RTS 失败")?;
    Ok(port)
}

/// 重连：重开串口 + 重新握手。
fn reconnect() -> Result<Box<dyn SerialPort>> {
    let mut p = open_port()?;
    handshake(&mut *p)?;
    Ok(p)
}

/// 握手：发 OnConnect → 读 29B 响应 → 验证回显 CRC；全 0/超时则重发（实测教训）。
fn handshake(port: &mut dyn SerialPort) -> Result<()> {
    // 清空收发缓冲区，避免残留数据干扰（实测 Python 版有 tcflush）
    port.clear(ClearBuffer::All).ok();

    let mut enc = FrameEncoder::new();
    for i in 0..3 {
        let frame = enc.on_connect();
        let sent_crc = u32::from_le_bytes(frame[0..4].try_into().unwrap());
        port.write_all(frame).context("写 OnConnect 失败")?;
        port.flush().ok();

        if let Some(resp_buf) = read_response(port, 3500) {
            let r = Response::parse(&resp_buf);
            tracing::debug!("握手响应[{}]: {:02x?}", i + 1, &resp_buf);
            if r.is_valid_handshake(sent_crc) {
                tracing::info!("握手成功（第 {} 次）", i + 1);
                return Ok(());
            }
        }
        tracing::warn!("握手响应无效（第 {} 次，可能全 0/超时），重发", i + 1);
        std::thread::sleep(Duration::from_millis(300));
    }
    anyhow::bail!("握手失败：3 次均未收到有效响应");
}

/// 读满 29B 响应；容忍分片，超时/读不满返回 None。
/// `timeout_ms` 控制整体等待时长（握手用长超时，主循环用短超时）。
fn read_response(port: &mut dyn SerialPort, timeout_ms: u64) -> Option<[u8; RESP_LEN]> {
    let mut buf = [0u8; RESP_LEN];
    let mut n = 0;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while n < RESP_LEN && Instant::now() < deadline {
        match port.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(_) => {} // 单次 read 超时（100ms）继续等，直到整体 deadline
        }
    }
    (n == RESP_LEN).then_some(buf)
}

/// 处理副屏命令：SubCmd::Set(1) 切 TDP 档，SubCmd::Reset(2) 复位到最高档。
/// 返回是否切了模式（供调用方决定是否立即回推新模式帧）。
///
/// 回弹根治：副屏触摸后要的是「确认帧」（pack[0x61]=新档位），不是「ryzenadj
/// 执行完」。故这里**先立即更新 PowerState**（主循环随即回推新档位帧），再把
/// ryzenadj 放到后台线程异步执行、读回实际瓦数后校准 tdp_watts。
fn handle_command(resp: &Response, power: &Arc<Mutex<PowerState>>) -> bool {
    match resp.sub_cmd {
        1 => {
            let mode = TdpMode::from_index(resp.sub_param).unwrap_or(TdpMode::AaaGame);
            tracing::info!("收到切档命令 mode_id={} -> {:?}", resp.sub_param, mode);
            apply_mode_update(power, mode);
            spawn_tdp_apply(mode, power);
            true
        }
        2 => {
            tracing::info!("收到复位命令，恢复 AAA Game");
            apply_mode_update(power, TdpMode::AaaGame);
            spawn_tdp_apply(TdpMode::AaaGame, power);
            true
        }
        _ => {
            tracing::debug!("忽略子命令: {}", resp.sub_cmd);
            false
        }
    }
}

/// 立即更新 PowerState 到目标档位（mode + 预设瓦数），供回推帧使用。
/// 纯状态更新，不触碰硬件、不阻塞。
fn apply_mode_update(power: &Arc<Mutex<PowerState>>, mode: TdpMode) {
    let mut p = power.lock().unwrap();
    p.mode = mode;
    p.tdp_watts = power_adapter::mode_stapm_watts(mode);
}

/// 后台执行 ryzenadj 切档（不阻塞回推），完成后读回实际瓦数校准 tdp_watts。
fn spawn_tdp_apply(mode: TdpMode, power: &Arc<Mutex<PowerState>>) {
    let power2 = Arc::clone(power);
    std::thread::spawn(move || match power_adapter::set_tdp_mode(mode) {
        Ok(()) => {
            let w = power_adapter::read_tdp_watts();
            let mut p = power2.lock().unwrap();
            p.tdp_watts = w;
            tracing::info!("后台已设置 TDP {:?} tdp={}W", mode, w);
        }
        Err(e) => tracing::error!("切 TDP 失败: {e:#}"),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Cmd, PowerState, TdpMode, RESP_LEN};

    /// 构造一条副屏 29B 响应帧（模拟副屏触摸切档），sub_cmd=Set(1)，sub_param=档位索引。
    fn build_response(sub_cmd: u32, sub_param: u32) -> Response {
        let mut buf = [0u8; RESP_LEN];
        buf[4] = Cmd::OnNormalInfo as u8;
        buf[20..24].copy_from_slice(&sub_cmd.to_le_bytes());
        buf[24..28].copy_from_slice(&sub_param.to_le_bytes());
        buf[28] = 0;
        Response::parse(&buf)
    }

    #[test]
    fn parse_29b_set_response() {
        let r = build_response(1, 3);
        assert_eq!(r.sub_cmd, 1);
        assert_eq!(r.sub_param, 3);
        assert_eq!(r.cmd, Cmd::OnNormalInfo as u8);
    }

    /// 纯状态更新：四档 mode + 预设瓦数映射正确（不触碰硬件）。
    #[test]
    fn apply_mode_update_sets_mode_and_watts() {
        let power = Arc::new(Mutex::new(PowerState {
            mode: TdpMode::AaaGame,
            tdp_watts: 0.0,
        }));
        apply_mode_update(&power, TdpMode::ClassicGame);
        let p = *power.lock().unwrap();
        assert_eq!(p.mode, TdpMode::ClassicGame);
        assert_eq!(p.tdp_watts, 40.0); // Classic stapm 40000mW

        apply_mode_update(&power, TdpMode::PcOffice);
        let p = *power.lock().unwrap();
        assert_eq!(p.mode, TdpMode::PcOffice);
        assert_eq!(p.tdp_watts, 20.0); // Office stapm 20000mW
    }

    /// 模拟「副屏 29B 响应注入 → handle_command → PowerState 更新」整条链。
    /// 断言 mode 更新（后台 ryzenadj 线程只校准 tdp_watts，不改 mode，断言稳定）。
    #[test]
    fn handle_command_updates_power_state() {
        let power = Arc::new(Mutex::new(PowerState {
            mode: TdpMode::AaaGame,
            tdp_watts: 45.0,
        }));
        let resp = build_response(1, 3); // Set -> Office
        assert!(handle_command(&resp, &power));
        let p = *power.lock().unwrap();
        assert_eq!(p.mode, TdpMode::PcOffice);

        let resp2 = build_response(2, 0); // Reset -> AAA
        assert!(handle_command(&resp2, &power));
        let p2 = *power.lock().unwrap();
        assert_eq!(p2.mode, TdpMode::AaaGame);
    }

    /// 档位索引映射：0=AAA/1=Classic/2=Retro/3=Office（与插件 PRESET_MODE_INDEX 一致）。
    #[test]
    fn four_mode_index_mapping() {
        assert_eq!(TdpMode::from_index(0), Some(TdpMode::AaaGame));
        assert_eq!(TdpMode::from_index(1), Some(TdpMode::ClassicGame));
        assert_eq!(TdpMode::from_index(2), Some(TdpMode::RetroGame));
        assert_eq!(TdpMode::from_index(3), Some(TdpMode::PcOffice));
        assert_eq!(TdpMode::from_index(4), None);
    }
}
