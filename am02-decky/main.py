"""AM02 Decky plugin backend.

Runs inside Decky Loader's Python sandbox. Because `plugin.json` has
`flags: ["root"]`, this process runs as root, which is required for
ryzenadj to touch the ryzen_smu kernel module / SMU registers.

MVP scope:
  * Four AYASpace-style TDP presets -> subprocess `ryzenadj`.
  * Manual TDP slider -> subprocess `ryzenadj`.
  * Display language / theme: PLACEHOLDER only (see `set_language` /
    `set_theme`). The real implementation will talk to `am02-service`
    (Rust, systemd, root) over IPC so IT writes pack[0x79]/pack[0x7a]
    to the side screen. This plugin must NOT touch the serial port
    directly.

Reference: SimpleDeckyTDP (aarron-lee/SimpleDeckyTDP) py_modules/ryzenadj.py
           https://github.com/aarron-lee/SimpleDeckyTDP
"""

import asyncio
import json
import os
import shutil
import socket
import subprocess

# --------------------------------------------------------------------------
# decky_plugin import: newer Decky exposes `decky_plugin` (used by
# SimpleDeckyTDP on @decky/ui 4.x / Bazzite); older Decky exposes `decky`.
# Fall back to a stderr logger so this file can be imported/tested
# standalone on the AM02 outside Decky.
# --------------------------------------------------------------------------
try:
    import decky_plugin as decky  # noqa: F401  (new Decky)
except Exception:
    try:
        import decky  # noqa: F401  (legacy Decky)
    except Exception:
        class _StubLogger:
            def info(self, *a, **k):
                print("[am02-decky]", *a, **k)

            def error(self, *a, **k):
                print("[am02-decky][error]", *a, **k)

        class _StubDecky:
            logger = _StubLogger()

        decky = _StubDecky()  # type: ignore

# --------------------------------------------------------------------------
# TDP presets (mW). Values are user-calibrated for the AYANEO AM02
# (Ryzen 7 7840HS), capped at 45W for presets; manual slider may exceed.
# DO NOT change these numbers.
# --------------------------------------------------------------------------
TDP_PRESETS = [
    {
        "id": "office",
        "label": "PC Office",
        "stapm": 20000,
        "fast": 25000,
        "slow": 15000,
    },
    {
        "id": "retro",
        "label": "Retro Game",
        "stapm": 30000,
        "fast": 35000,
        "slow": 25000,
    },
    {
        "id": "classic",
        "label": "Classic Game",
        "stapm": 40000,
        "fast": 45000,
        "slow": 35000,
    },
    {
        "id": "aaa",
        "label": "AAA Game",
        "stapm": 45000,
        "fast": 50000,
        "slow": 40000,
    },
]

# Manual slider range (mW). ryzenadj caps out around 54W.
MANUAL_MIN_MW = 5000
MANUAL_MAX_MW = 54000
DEFAULT_MANUAL_MW = 30000

# Placeholder for future am02-service IPC (side-screen pack[] fields).
# pack[0x79] = language (0 = Chinese, 1 = English)
# pack[0x7a] = theme    (0 = black, 1 = white)
LANG_ZH = 0
LANG_EN = 1
THEME_BLACK = 0
THEME_WHITE = 1

# pack[0x84] = 12/24 小时制标志位（0 = 12 小时制, 1 = 24 小时制）
TIME_FORMAT_12H = 0
TIME_FORMAT_24H = 1

# am02-service IPC socket（与 Rust 侧 ipc.rs 保持一致，可用环境变量覆盖）
SERVICE_SOCKET_PATH = os.environ.get("AM02_SERVICE_SOCK", "/run/am02-service.sock")

# 档位 id -> am02-service TdpMode 索引（与 Rust 侧 protocol.rs 的 TdpMode 枚举一致）：
#   AaaGame=0 / ClassicGame=1 / RetroGame=2 / PcOffice=3
# decky 的 TDP_PRESETS 顺序是 office/retro/classic/aaa，需映射到上述索引。
PRESET_MODE_INDEX = {
    "office": 3,
    "retro": 2,
    "classic": 1,
    "aaa": 0,
}

# 反向映射：am02-service 的 mode 索引 -> decky 档位 id。
# 用于 get_state 反向同步（副屏触摸切档后，插件 UI 能回显对应档位）。
MODE_INDEX_PROFILE = {0: "aaa", 1: "classic", 2: "retro", 3: "office"}


def _nearest_mode_index(mw: int) -> int:
    """手动瓦数 -> 最接近的预设档位索引。

    副屏 pack[0x61] 只有四档（无 manual 档），手动滑条切完后用它把档位
    勾选就近落到某一档，同时 pack[0x69] 显示实际手动瓦数。
    """
    best = "aaa"
    best_diff = None
    for p in TDP_PRESETS:
        diff = abs(p["stapm"] - mw)
        if best_diff is None or diff < best_diff:
            best_diff = diff
            best = p["id"]
    return PRESET_MODE_INDEX[best]

_SETTINGS_PATH = os.path.expanduser("~/.config/am02-decky/settings.json")

_DEFAULT_STATE = {
    "currentProfile": "aaa",  # one of preset id or "manual"
    "manualTdpMw": DEFAULT_MANUAL_MW,
    "language": LANG_ZH,
    "theme": THEME_BLACK,
    "timeFormat": TIME_FORMAT_24H,  # 0 = 12h, 1 = 24h
    "location": "",  # 天气地区（城市名）；空串 = 关闭天气
}

_state: dict = {}


def _load_state() -> dict:
    try:
        with open(_SETTINGS_PATH, "r", encoding="utf-8") as f:
            loaded = json.load(f)
    except Exception:
        loaded = {}
    merged = dict(_DEFAULT_STATE)
    merged.update(loaded)
    return merged


def _save_state() -> None:
    try:
        os.makedirs(os.path.dirname(_SETTINGS_PATH), exist_ok=True)
        with open(_SETTINGS_PATH, "w", encoding="utf-8") as f:
            json.dump(_state, f, ensure_ascii=False, indent=2)
    except Exception as e:
        decky.logger.error(f"am02-decky: failed to persist settings: {e}")


def _find_ryzenadj() -> str | None:
    """Locate ryzenadj, preferring PATH then common locations."""
    path = shutil.which("ryzenadj")
    if path:
        return path
    for cand in (
        os.path.expanduser("~/.local/bin/ryzenadj"),
        os.path.expanduser("~/.nix-profile/bin/ryzenadj"),
        os.path.expanduser("~/homebrew/plugins/am02-decky/bin/ryzenadj"),
    ):
        if os.path.exists(cand):
            return cand
    return None


def _run_ryzenadj(stapm_mw: int, fast_mw: int, slow_mw: int) -> dict:
    """Call `ryzenadj --stapm-limit/--fast-limit/--slow-limit` (mW)."""
    path = _find_ryzenadj()
    if not path:
        decky.logger.error("am02-decky: ryzenadj not found in PATH")
        return {"ok": False, "returncode": -1, "stderr": "ryzenadj not found"}

    cmd = [
        path,
        "--stapm-limit", str(stapm_mw),
        "--fast-limit", str(fast_mw),
        "--slow-limit", str(slow_mw),
    ]
    decky.logger.info(f"am02-decky: {cmd}")

    try:
        # list form (no shell=True) avoids shell-injection surface; values
        # are ints anyway. LD_LIBRARY_PATH cleared like SimpleDeckyTDP does.
        env = os.environ.copy()
        env["LD_LIBRARY_PATH"] = ""
        proc = subprocess.run(
            cmd,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
        )
        ok = proc.returncode == 0
        if not ok:
            decky.logger.error(
                f"am02-decky: ryzenadj failed rc={proc.returncode} "
                f"stderr={proc.stderr.strip()}"
            )
        return {"ok": ok, "returncode": proc.returncode, "stderr": proc.stderr}
    except Exception as e:
        decky.logger.error(f"am02-decky: ryzenadj exception: {e}")
        return {"ok": False, "returncode": -1, "stderr": str(e)}


def _ipc_send(cmd: dict) -> bool:
    """Send a JSON command to am02-service over its unix socket.

    am02-service is the only process that owns /dev/ttyS0 and writes
    pack[0x79]/pack[0x7a] to the side screen. This plugin must NOT touch
    the serial port directly — it only relays via this IPC socket.

    Returns True on success. On any failure logs an error and returns
    False (graceful degradation — never raises).
    """
    payload = json.dumps(cmd) + "\n"
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(2.0)
            s.connect(SERVICE_SOCKET_PATH)
            s.sendall(payload.encode("utf-8"))
        return True
    except Exception as e:
        decky.logger.error(
            f"am02-decky: IPC send to {SERVICE_SOCKET_PATH} failed: {e}"
        )
        return False


def _ipc_request(cmd: dict, timeout: float = 0.5) -> dict | None:
    """Send a JSON command and read back a JSON reply (for `get_state`/`get_location`/`search_city`).

    am02-service replies to these with JSON; the set_* commands do not reply
    (read times out -> returns None). Never raises — on any failure logs and
    returns None. `timeout` is the read timeout in seconds (IP 定位需更长的等待).
    """
    payload = json.dumps(cmd) + "\n"
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(1.0)
            s.connect(SERVICE_SOCKET_PATH)
            s.sendall(payload.encode("utf-8"))
            s.settimeout(timeout)
            data = b""
            try:
                while b"\n" not in data and len(data) < 4096:
                    chunk = s.recv(4096)
                    if not chunk:
                        break
                    data += chunk
            except socket.timeout:
                pass
            text = data.decode("utf-8", errors="replace").strip()
            if text:
                return json.loads(text)
    except Exception as e:
        decky.logger.error(
            f"am02-decky: IPC request to {SERVICE_SOCKET_PATH} failed: {e}"
        )
    return None


def get_service_state() -> dict | None:
    """Read the current full state from am02-service.

    This is the reverse-sync channel: when the user switches TDP on the
    side screen (touch), am02-service's PowerState changes and `get_state`
    returns the new mode, so the plugin UI can follow.
    """
    return _ipc_request({"op": "get_state"})


class Plugin:
    async def get_tdp_profiles(self):
        return TDP_PRESETS

    async def get_settings(self):
        # 反向同步：每次读取设置时，先从 am02-service 拉一次真实状态，
        # 让 UI 反映副屏触摸切档后的档位/语言/主题/时间制式/地区。
        svc = get_service_state()
        if svc:
            mode = svc.get("mode")
            if mode in MODE_INDEX_PROFILE:
                _state["currentProfile"] = MODE_INDEX_PROFILE[mode]
            if "language" in svc:
                _state["language"] = int(svc["language"])
            if "theme" in svc:
                _state["theme"] = int(svc["theme"])
            if "time_format" in svc:
                _state["timeFormat"] = int(svc["time_format"])
            if "location" in svc:
                svc_loc = svc["location"] or ""
                # 防御：服务端 location 为空时，不覆盖本地已设置的城市（避免反向同步清空）
                if svc_loc or not _state.get("location"):
                    _state["location"] = svc_loc
        return {
            **_state,
            "manualMinMw": MANUAL_MIN_MW,
            "manualMaxMw": MANUAL_MAX_MW,
        }

    async def set_tdp_profile(self, profile_id: str) -> bool:
        preset = next((p for p in TDP_PRESETS if p["id"] == profile_id), None)
        if not preset:
            decky.logger.error(f"am02-decky: unknown profile {profile_id}")
            return False
        result = _run_ryzenadj(preset["stapm"], preset["fast"], preset["slow"])
        if result["ok"]:
            _state["currentProfile"] = profile_id
            _save_state()
            # 同步到副屏：mode 用映射后的档位索引，tdp_watts 用该档 STAPM 瓦数
            # （pack[0x69]）。socket 连不上不 crash，_ipc_send 内部已吞异常。
            _ipc_send(
                {
                    "op": "set_power_mode",
                    "mode": PRESET_MODE_INDEX[profile_id],
                    "tdp_watts": preset["stapm"] / 1000,
                }
            )
        return result["ok"]

    async def set_manual_tdp(self, mw: int) -> bool:
        mw = max(MANUAL_MIN_MW, min(MANUAL_MAX_MW, int(mw)))
        # Manual slider sets all three limits equal (same approach as
        # SimpleDeckyTDP's ryzenadj.set_tdp).
        result = _run_ryzenadj(mw, mw, mw)
        if result["ok"]:
            _state["currentProfile"] = "manual"
            _state["manualTdpMw"] = mw
            _save_state()
            # 手动档无对应副屏四档：档位就近落到最近预设，瓦数推实际值
            _ipc_send(
                {
                    "op": "set_power_mode",
                    "mode": _nearest_mode_index(mw),
                    "tdp_watts": mw / 1000,
                }
            )
        return result["ok"]

    # ------------------------------------------------------------------
    # Display language / theme: relay to `am02-service` over unix socket IPC.
    # am02-service owns the serial port and writes pack[0x79]/pack[0x7a]
    # to the side screen. This plugin must NOT touch the serial port.
    # ------------------------------------------------------------------
    async def set_language(self, lang: int) -> bool:
        lang = int(lang)
        ok = _ipc_send({"op": "set_language", "lang": lang})
        if ok:
            _state["language"] = lang
            _save_state()
        return ok

    async def set_theme(self, theme: int) -> bool:
        theme = int(theme)
        ok = _ipc_send({"op": "set_theme", "theme": theme})
        if ok:
            _state["theme"] = theme
            _save_state()
        return ok

    # ------------------------------------------------------------------
    # Time format (12h / 24h): relay to `am02-service` over unix socket IPC.
    # am02-service owns the serial port and writes pack[0x84] (0 = 12h,
    # 1 = 24h) to the side screen.
    # ------------------------------------------------------------------
    async def set_time_format(self, fmt: int) -> bool:
        fmt = int(fmt)
        if fmt not in (TIME_FORMAT_12H, TIME_FORMAT_24H):
            decky.logger.error(f"am02-decky: invalid time format {fmt}")
            return False
        ok = _ipc_send({"op": "set_time_format", "format": fmt})
        if ok:
            _state["timeFormat"] = fmt
            _save_state()
        return ok

    # ------------------------------------------------------------------
    # Weather location: relay to `am02-service` over unix socket IPC.
    # am02-service fetches OpenWeatherMap weather and writes the pack
    # weather string fields (0x85 city / 0x95 weather / 0xc5 temp /
    # 0xd1 wind direction / 0xdd wind power) to the side screen.
    # ------------------------------------------------------------------
    async def set_location(self, city: str) -> bool:
        city = (city or "").strip()
        ok = _ipc_send({"op": "set_location", "city": city})
        if ok:
            _state["location"] = city
            _save_state()
        return ok

    # ------------------------------------------------------------------
    # Weather location helper: 城市模糊搜索，走 am02-service IPC（会回写 JSON）。
    # ------------------------------------------------------------------
    async def search_city(self, query: str) -> dict:
        return _ipc_request({"op": "search_city", "query": (query or "")}) or {
            "ok": False,
            "cities": [],
        }

    # Asyncio-compatible long-running code, executed in a task on load.
    async def _main(self):
        global _state
        _state = _load_state()
        decky.logger.info("am02-decky starting")

    async def _unload(self):
        decky.logger.info("am02-decky unloading")

    async def _uninstall(self):
        decky.logger.info("am02-decky uninstalling")
