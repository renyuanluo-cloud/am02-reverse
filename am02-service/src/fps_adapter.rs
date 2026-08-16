//! P4：FPS 适配器。
//!
//! gamescope（Bazzite-Deck Game Mode）每帧通过 System V 消息队列推送一条
//! `mangoapp_msg_v1`，本模块读 `app_frametime_ns` 换算 FPS = 1e9 / app_frametime_ns。
//!
//! 队列 key = ftok("mangoapp", 65)，消息类型 msg_type = 1，非阻塞读。
//! Linux 下用 libc 调 msgget/msgrcv/ftok；非 Linux（本机 Windows）编译成返回 0 的空实现。

#[cfg(target_os = "linux")]
mod imp {
    use std::ffi::CString;

    /// gamescope 消息类型（msgrcv 的 msgtyp）
    const MSG_TYPE: libc::c_long = 1;
    /// ftok 的 proj_id（'A' = 65）
    const PROJ_ID: libc::c_int = 65;
    /// 接收缓冲区消息数据部分大小，足够容纳 v1(104B)/v2(约 384B)
    const MSGSZ: usize = 1024;
    /// app_frametime_ns 在缓冲区内的绝对偏移（含 8B msg_type 前缀）
    const APP_FRAMETIME_OFF: usize = 24;

    /// 与 gamescope src/mangoapp.cpp 的 `mangoapp_msg_v1` 对应（Linux x86_64 自然对齐）。
    /// 显式 padding 字段保证 `#[repr(C)]` 布局与 C 完全一致，总大小 112B。
    /// 仅用于文档化布局 + 编译期 size 断言；运行时解析用字节偏移（APP_FRAMETIME_OFF），
    /// 避免 transmute 踩 padding / 平台 `long` 宽度差异。
    #[repr(C)]
    #[allow(dead_code)]
    struct MangoappMsgV1 {
        msg_type: libc::c_long,    // 0
        version: u32,              // 8
        _pad_hdr: u32,             // 12  (header 对齐到 8)
        pid: u32,                  // 16
        _pad_pid: u32,             // 20  (u64 对齐)
        app_frametime_ns: u64,     // 24
        fsr_upscale: u8,           // 32
        fsr_sharpness: u8,         // 33
        _pad_fsr: [u8; 6],         // 34  (u64 对齐)
        visible_frametime_ns: u64, // 40
        latency_ns: u64,           // 48
        output_width: u32,         // 56
        output_height: u32,        // 60
        display_refresh: u32,      // 64
        app_wants_hdr: u8,         // 68  (C bool)
        steam_focused: u8,         // 69  (C bool)
        _pad_flags: [u8; 2],       // 70
        engine_name: [u8; 40],     // 70
    }

    const _: () = assert!(std::mem::size_of::<MangoappMsgV1>() == 112);

    /// 读当前帧 FPS；连接失败 / 无新帧 / 解析异常一律返回 0。
    pub fn read_fps() -> i32 {
        // 首选 ftok 直连 gamescope 队列
        if let Some(id) = ftok_connect() {
            return read_frame(id).unwrap_or(0);
        }
        // 回退：枚举 /proc/sysvipc/msg（等价 ipcs -q）逐个试读
        enumerate_read().unwrap_or(0)
    }

    /// ftok("mangoapp", 65) -> msgget(key, 0666)。
    /// ftok 要求当前工作目录存在名为 `mangoapp` 的文件，否则返回 -1。
    fn ftok_connect() -> Option<libc::c_int> {
        let path = CString::new("mangoapp").ok()?;
        let key = unsafe { libc::ftok(path.as_ptr(), PROJ_ID) };
        if key == -1 {
            return None;
        }
        let id = unsafe { libc::msgget(key, 0o666) };
        if id < 0 {
            None
        } else {
            Some(id)
        }
    }

    /// 枚举所有 SysV 消息队列，逐个非阻塞试读，返回第一条有效消息的 FPS。
    /// /proc/sysvipc/msg 各列：key msqid perms cbytes qnum lspid lrpid uid gid ...
    fn enumerate_read() -> Option<i32> {
        let s = std::fs::read_to_string("/proc/sysvipc/msg").ok()?;
        for line in s.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 2 {
                continue;
            }
            let Ok(msqid) = cols[1].parse::<libc::c_int>() else {
                continue;
            };
            if let Some(fps) = read_frame(msqid) {
                return Some(fps);
            }
        }
        None
    }

    /// 从指定队列非阻塞读一条 msg_type=1 的消息，解析 app_frametime_ns -> FPS。
    /// 无新消息 / 读失败返回 None；读到但帧时间为 0 返回 Some(0)（除零保护）。
    fn read_frame(msgid: libc::c_int) -> Option<i32> {
        let mut buf = [0u8; MSGSZ + std::mem::size_of::<libc::c_long>()];
        let r = unsafe {
            libc::msgrcv(
                msgid,
                buf.as_mut_ptr() as *mut libc::c_void,
                MSGSZ as libc::size_t,
                MSG_TYPE,
                libc::IPC_NOWAIT | libc::MSG_NOERROR,
            )
        };
        if r < 0 {
            return None; // ENOMSG(无新帧) / EINVAL / EACCES / EIDRM
        }
        let ft =
            u64::from_le_bytes(buf[APP_FRAMETIME_OFF..APP_FRAMETIME_OFF + 8].try_into().ok()?);
        if ft == 0 {
            Some(0)
        } else {
            Some((1_000_000_000u64 / ft) as i32)
        }
    }
}

#[cfg(target_os = "linux")]
pub fn read_fps() -> i32 {
    imp::read_fps()
}

#[cfg(not(target_os = "linux"))]
pub fn read_fps() -> i32 {
    0
}
