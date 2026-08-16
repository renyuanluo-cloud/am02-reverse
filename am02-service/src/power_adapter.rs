//! P3：功耗控制适配器。
//!
//! 通过调用 ryzenadj（ryzen_smu 内核模块已装）设置/读取 TDP。
//! 部署时服务以 root 运行，直接调 ryzenadj，不带 sudo。

use std::process::Command;

use anyhow::{Context, Result};

use crate::protocol::TdpMode;

/// 单档 TDP 限值（单位：毫瓦）
#[derive(Debug, Clone, Copy)]
struct TdpLimits {
    stapm: u32,
    fast: u32,
    slow: u32,
}

/// 四档 TDP 映射表（用户已标定，封顶 45W；后续实测微调）
fn tdp_limits(mode: TdpMode) -> TdpLimits {
    match mode {
        TdpMode::PcOffice => TdpLimits {
            stapm: 20_000,
            fast: 25_000,
            slow: 15_000,
        },
        TdpMode::RetroGame => TdpLimits {
            stapm: 30_000,
            fast: 35_000,
            slow: 25_000,
        },
        TdpMode::ClassicGame => TdpLimits {
            stapm: 40_000,
            fast: 45_000,
            slow: 35_000,
        },
        TdpMode::AaaGame => TdpLimits {
            stapm: 45_000,
            fast: 50_000,
            slow: 40_000,
        },
    }
}

/// 档位对应的 STAPM 瓦数（供立即更新 PowerState.tdp_watts 用，不回读硬件）。
///
/// 副屏触摸切档时，主循环需要**立即**回推 pack[0x61]/pack[0x69]（副屏确认
/// 不弹回），此时不能等 ryzenadj 执行完再读回实际值，故用预设值先行填充。
pub fn mode_stapm_watts(mode: TdpMode) -> f32 {
    tdp_limits(mode).stapm as f32 / 1000.0
}

/// 设置 TDP 档位：`ryzenadj --stapm-limit=<mw> --fast-limit=<mw> --slow-limit=<mw>`。
/// 检查退出状态，失败返回错误。
pub fn set_tdp_mode(mode: TdpMode) -> Result<()> {
    let l = tdp_limits(mode);
    let status = Command::new("ryzenadj")
        .arg(format!("--stapm-limit={}", l.stapm))
        .arg(format!("--fast-limit={}", l.fast))
        .arg(format!("--slow-limit={}", l.slow))
        .status()
        .context("执行 ryzenadj 失败（确认已装 ryzenadj 且以 root 运行）")?;

    if !status.success() {
        anyhow::bail!("ryzenadj 返回非零退出码: {:?}", status.code());
    }

    tracing::info!(
        "已设置 TDP {:?}: stapm={} fast={} slow={} mW",
        mode,
        l.stapm,
        l.fast,
        l.slow
    );
    Ok(())
}

/// 读当前 STAPM 限值（W），解析 `ryzenadj -i` 输出中的 "STAPM LIMIT" 行（回退 "STAPM VALUE"）。
/// 解析失败或 ryzenadj 不可用返回 0，不阻塞主流程。
pub fn read_tdp_watts() -> f32 {
    let out = match Command::new("ryzenadj").arg("-i").output() {
        Ok(o) if o.status.success() => o.stdout,
        _ => return 0.0,
    };
    let text = String::from_utf8_lossy(&out);

    // 优先读配置限值（STAPM LIMIT），其次读当前值（STAPM VALUE）
    for key in ["STAPM LIMIT", "STAPM VALUE"] {
        for line in text.lines() {
            let upper = line.to_ascii_uppercase();
            if upper.contains(key) {
                // 表格式输出：第一列是瓦数值（如 35.000），随后才是毫瓦参数（35000）
                if let Some(v) = upper
                    .split_whitespace()
                    .find_map(|t| t.parse::<f32>().ok())
                {
                    return v;
                }
            }
        }
    }
    0.0
}
