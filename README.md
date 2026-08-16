# AYANEO AM02 副屏复刻（am02-reverse）

> 让 AYANEO AM02 mini PC 的副屏在 **Linux / Bazzite** 下复活：复刻原厂 AYASpace 的副屏显示与功耗控制，并以 **Decky 插件 + 后台服务** 的形式交付，与 Steam 游戏模式（gamepadui）无缝集成。

---

## 这是什么

AYANEO AM02 是一台带 **2.8 寸副屏** 的 mini PC。原厂通过 Windows 下的 AYASpace 软件驱动副屏（显示 CPU/GPU/温度/功耗/网速/风扇/天气/时钟，并支持功耗档位切换）。

本项目**在不依赖 AYASpace 的前提下**，逆向其副屏串口协议，在 Linux（Bazzite-Deck）上复刻了整套功能：

- **后台服务 `am02-service`**（Rust，root systemd 服务）：独占副屏串口，采集系统信息并编码成 253 字节帧推给副屏；通过 Unix socket 提供 IPC 给插件。
- **Decky 插件 `am02-decky`**（React 前端 + Python 后端）：在 Steam 快捷菜单（QAM）里提供 TDP 档位、手动功耗、语言/主题、12/24 小时制、天气城市等设置界面。

---

## 功能特性

| 模块 | 内容 |
|------|------|
| 副屏数据 | CPU/GPU 使用率、温度、功耗、频率，RAM，磁盘，风扇转速，网速，TDP，FPS |
| 功耗控制 | 四档 TDP 预设（办公/复古/经典/3A）+ 手动功耗（STAPM/FAST/SLOW），插件与副屏**双向同步** |
| 显示设置 | 语言（中/英）、主题（深/浅）、12/24 小时制，即时下发到副屏 |
| 天气 | 中国县级以上城市（含市辖区）本地映射，OpenWeatherMap 拉取，副屏显示城市/天气/气温/风力 |
| 时钟 | 每帧同步本地时间（北京时区） |
| 风扇 | 通过 `it87` 驱动读 IT8620E Super-IO 芯片真实转速 |

---

## 架构

```
┌─────────────────────────────────────────────────────┐
│  Steam 游戏模式（gamepadui / gamescope）              │
│  ┌──────────────────────────────────────────────┐   │
│  │  Decky Loader → AM02 Decky 插件（React UI）   │   │
│  └──────────────┬───────────────────────────────┘   │
│                 │  Unix socket IPC（JSON）           │
│  ┌──────────────▼───────────────────────────────┐   │
│  │  am02-service（Rust，root systemd 服务）       │   │
│  │  · 采集系统信息  · 编码 253B 帧  · IPC 接收    │   │
│  └──────────────┬───────────────────────────────┘   │
│                 │  /dev/ttyS0（串口）                │
└─────────────────▼───────────────────────────────────┘
            副屏 MCU（2.8 寸屏）
```

- **插件绝不碰串口**：语言/主题/功耗都通过 IPC 下发，由 am02-service 统一编码推帧，避免串口竞争。
- **协议 v2**：253 字节帧 + CRC32，字段偏移由 AYASpaceCef.exe 与副屏固件双源逆向标定。

---

## 硬件要求

| 项 | 要求 |
|----|------|
| 设备 | AYANEO AM02 mini PC（带副屏） |
| 系统 | **Bazzite-Deck**（或 SteamOS），x86_64 |
| 串口 | `/dev/ttyS0`（副屏通信，硬依赖） |
| 内核模块 | `it87`（主线内核自带）、`ryzen_smu`（第三方，见下） |
| 工具 | `ryzenadj`（功耗控制） |

> ⚠️ **仅适用带副屏的 AM02**。脚本会先检测 `/dev/ttyS0`，不存在则直接报错退出，不会误装到别的机器。

---

## 安装

### 一键安装（推荐）

在 Bazzite 的桌面模式终端里执行：

```bash
curl -fsSL https://github.com/renyuanluo-cloud/am02-reverse/releases/latest/download/install.sh | sudo bash
```

脚本会自动完成：内核模块（ryzen_smu + it87）→ ryzenadj → 后台服务 → exfat 挂载 → Decky 插件部署，全程带日志与失败回滚。

### 手动安装（分步）

1. 下载 Release 里的 `am02-setup-<ver>.tar.gz` 并解压。
2. 终端进入解压目录，执行：

   ```bash
   sudo bash install.sh
   ```

3. 看到 `安装完成` 后**重启机器**，Steam 快捷菜单（`…` 键 / `Ctrl+2`）里出现「AM02 Decky」。

### 卸载

```bash
sudo bash uninstall.sh
```

---

## 环境适配（务必阅读）

这台机器有一个**非标准之处**：AM02 是 mini PC，却跑着**掌机版** Bazzite，默认输出参数指向不存在的内置屏 `eDP-1`，会导致 gamescope 反复「找 eDP → 回退 HDMI」，切换模式时黑屏、过夜后大屏界面渲染进程丢失。

**安装后必须手动把默认输出从 DP/eDP 改成 HDMI**，详见 [docs/环境适配说明.md](docs/环境适配说明.md)。核心一句话：

```
mkdir -p ~/.config/environment.d
echo 'OUTPUT_CONNECTOR=*,HDMI-A-1' > ~/.config/environment.d/gamescope-output.conf
```

---

## 第三方依赖与致谢

| 依赖 | 用途 | 许可证 |
|------|------|--------|
| [amkillam/ryzen_smu](https://github.com/amkillam/ryzen_smu) | 内核模块，读写 AMD SMU 功耗/频率 | GPL-2.0 |
| [FlyGoat/RyzenAdj](https://github.com/FlyGoat/RyzenAdj) | TDP 控制 CLI | LGPL-3.0 |
| [OpenWeatherMap](https://openweathermap.org) | 天气数据 | 免费档 |
| it87（主线内核） | 风扇转速（IT8620E） | GPL |

> ryzen_smu 与 ryzenadj 为第三方项目，本仓库不包含其源码，安装脚本会按需拉取/编译；预编译内核模块随 Release 产物分发。

---

## 免责声明

本项目为**个人学习与研究**用途，通过逆向工程方式复刻 AYANEO AM02 副屏通信协议。所有代码仅用于在自己的设备上替代原厂工具、改善 Linux 使用体验：

- 不包含、不分发任何 AYANEO 专有代码或固件。
- 不保证在非 AM02 设备上的可用性，内核模块加载与功耗调整有风险，请自行承担。
- 与 AYANEO 公司无任何关联，商标归其各自所有者所有。

---

## 许可证

本项目代码采用 [BSD-3-Clause](LICENSE) 许可证。
