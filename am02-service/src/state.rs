//! AM02 服务状态机（设计文档 §8）。
//!
//! transition(state, event) -> (new_state, action)

use crate::protocol::SubCmd;

/// 服务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Connecting { retry: u8 },
    Connected,
    Running,
    Reconnecting { retry: u8 },
}

/// 触发状态转移的事件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// 串口打开成功
    SerialOpened,
    /// 握手成功（回显 CRC 匹配）
    HandshakeOk,
    /// 握手失败（全 0 响应 / 超时）
    HandshakeFailed,
    /// 1s 周期定时器触发（表达 §8 转移表「Connected --定时器--> Running」）
    TimerTick,
    /// 收到副屏子命令（SubCmd + 参数）
    SubCmdReceived(SubCmd, u32),
    /// 串口错误
    SerialError,
    /// 关闭服务
    Shutdown,
}

/// 状态转移附带动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    /// 发 OnConnect
    SendOnConnect,
    /// 重发 OnConnect
    ResendOnConnect,
    /// 启动 1s 定时器
    StartTimer,
    /// 发 OnNormalInfo
    SendNormalInfo,
    /// 退避 5s
    Backoff,
    /// 调 ryzenadj 切 TDP（携带模式 ID）
    SetTdp(u32),
    /// 复位模式
    ResetMode,
    /// 关串口、释放资源
    ShutdownPort,
}

/// 状态转移表（§8）：返回 (新状态, 动作)
pub fn transition(state: State, event: Event) -> (State, Action) {
    use Action::*;
    use State::*;

    match (state, event) {
        // Idle --SerialOpened--> Connecting，发 OnConnect
        (Idle, Event::SerialOpened) => (Connecting { retry: 0 }, SendOnConnect),

        // Connecting --HandshakeOk--> Connected，启动定时器
        (Connecting { retry: _ }, Event::HandshakeOk) => (Connected, StartTimer),
        // Connecting --HandshakeFailed(retry<3)--> Connecting，重发 OnConnect
        (Connecting { retry }, Event::HandshakeFailed) if retry < 3 => {
            (Connecting { retry: retry + 1 }, ResendOnConnect)
        }
        // Connecting --HandshakeFailed(retry>=3)--> Reconnecting，退避 5s
        (Connecting { retry: _ }, Event::HandshakeFailed) => {
            (Reconnecting { retry: 0 }, Backoff)
        }

        // Connected --定时器--> Running，发 OnNormalInfo
        (Connected, Event::TimerTick) => (Running, SendNormalInfo),

        // Running --SubCmdReceived(Set)--> Running，切 TDP 并回传 pack[0xdd]
        (Running, Event::SubCmdReceived(SubCmd::Set, mode_id)) => (Running, SetTdp(mode_id)),
        // Running --SubCmdReceived(Reset)--> Running，复位模式
        (Running, Event::SubCmdReceived(SubCmd::Reset, _)) => (Running, ResetMode),
        // Running --SerialError--> Reconnecting，重发 OnConnect
        (Running, Event::SerialError) => (Reconnecting { retry: 0 }, ResendOnConnect),

        // Reconnecting --HandshakeOk--> Connected，恢复定时器
        (Reconnecting { retry: _ }, Event::HandshakeOk) => (Connected, StartTimer),
        // Reconnecting --HandshakeFailed(retry<3)--> Reconnecting，重发 OnConnect
        (Reconnecting { retry }, Event::HandshakeFailed) if retry < 3 => {
            (Reconnecting { retry: retry + 1 }, ResendOnConnect)
        }
        // Reconnecting --HandshakeFailed(retry>=3)--> Reconnecting，退避 5s
        (Reconnecting { retry: _ }, Event::HandshakeFailed) => {
            (Reconnecting { retry: 0 }, Backoff)
        }

        // * --Shutdown--> Idle，关串口
        (_, Event::Shutdown) => (Idle, ShutdownPort),

        // 未定义组合：保持原状态
        (s, _) => (s, None),
    }
}
