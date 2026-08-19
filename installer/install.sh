#!/usr/bin/env bash
# =============================================================================
#  AYANEO AM02 副屏复刻 —— 一键安装脚本
#  用法: curl -fsSL <release>/install.sh | sudo bash
#        （或解压 am02-setup-<ver>.tar.gz 后 sudo bash install.sh）
#  要求: root 运行；目标机为 Bazzite-Deck / SteamOS；x86_64；存在 /dev/ttyS0
#  幂等: 可重复运行（升级 = 下载新版本重跑一遍）
# =============================================================================
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSION="$(cat "$SCRIPT_DIR/VERSION" 2>/dev/null || echo "unknown")"
INSTALL_ROOT="/var/lib/am02-service"
SRC_BIN="$SCRIPT_DIR/bin"
SRC_MOD="$SCRIPT_DIR/modules"
SRC_CFG="$SCRIPT_DIR/config"
SRC_PLUGIN="$SCRIPT_DIR/plugin/am02-decky"
LOG="/tmp/am02-setup.log"
ROLLBACK_ACTIONS=()

log()  { echo "[am02-setup] $*" | tee -a "$LOG"; }
fail() { echo "[am02-setup][FAIL] $*" | tee -a "$LOG" >&2; exit 1; }
step() { echo; echo "==> [$1] $2" | tee -a "$LOG"; }

rollback() {
  echo "[am02-setup] 开始回滚..." | tee -a "$LOG"
  for ((i=${#ROLLBACK_ACTIONS[@]}-1; i>=0; i--)); do
    echo "  - ${ROLLBACK_ACTIONS[i]}" | tee -a "$LOG"
    eval "${ROLLBACK_ACTIONS[i]}" || true
  done
  echo "[am02-setup] 回滚完成。" | tee -a "$LOG"
}
trap 'log "脚本异常退出"; rollback; exit 1' ERR

# 解析真实（非 root）用户：Decky 插件要装进用户 home
resolve_plugin_user() {
  [ -n "${SUDO_USER:-}" ] && { echo "$SUDO_USER"; return; }
  for u in deck gamer luorenyuan; do id "$u" >/dev/null 2>&1 && { echo "$u"; return; }; done
  find /home -maxdepth 2 -type d -name homebrew 2>/dev/null | head -1 | xargs stat -c '%U' 2>/dev/null
}

echo "[am02-setup] AYANEO AM02 副屏复刻 安装脚本 v$VERSION" | tee "$LOG"
[ "$(id -u)" -eq 0 ] || fail "请用 sudo 运行：curl ... | sudo bash"

# ── Step 0 前置检测 ──────────────────────────────────────────────
step 0 "前置检测"
. /etc/os-release
case "${ID:-}${VARIANT_ID:-}" in
  *bazzite*|*steamos*) : ;;
  *) fail "不支持的发行版（检测到 ${ID:-unknown}）。仅支持 Bazzite-Deck / SteamOS。" ;;
esac
[ "$(uname -m)" = "x86_64" ] || fail "仅支持 x86_64（当前 $(uname -m)）"
[ -e /dev/ttyS0 ] || fail "未找到 /dev/ttyS0 —— 本工具仅适用于带副屏的 AYANEO AM02"
log "OS=${ID}  arch=x86_64  串口=/dev/ttyS0  OK"

# SteamOS 只读根解锁（Bazzite 根可写无需处理）
case "${ID:-}" in
  *steamos*)
    steamos-readonly disable 2>/dev/null \
      || log "提示：steamos-readonly 解锁失败（可能已可写）"
    ;;
esac

# ── Step 1 ryzen_smu 内核模块 ─────────────────────────────────────
step 1 "ryzen_smu 内核模块（AMD SMU 功耗/频率）"
mkdir -p "$INSTALL_ROOT/modules"
KREL="$(uname -r)"
KO_PREBUILT="$SRC_MOD/ryzen_smu.ko"

if [ -f "$KO_PREBUILT" ]; then
  log "使用包内预编译 ryzen_smu.ko"
  cp "$KO_PREBUILT" "$INSTALL_ROOT/modules/ryzen_smu.ko"
  ROLLBACK_ACTIONS+=("rm -f $INSTALL_ROOT/modules/ryzen_smu.ko")
else
  # 兜底：从上游编译（需 kernel-devel + gcc/make）
  log "包内无预编译 .ko，尝试从上游编译（amkillam/ryzen_smu）"
  command -v git >/dev/null 2>&1 && command -v make >/dev/null 2>&1 \
    || fail "缺少 git/make，且无预编译模块。请先安装编译工具链。"
  [ -d "/lib/modules/$KREL/build" ] || fail "缺少内核头 /lib/modules/$KREL/build。Bazzite: rpm-ostree install kernel-devel 后重启再试。"
  TMP="$(mktemp -d)"
  git clone --depth 1 https://github.com/amkillam/ryzen_smu.git "$TMP/ryzen_smu" 2>&1 | tail -2
  ( cd "$TMP/ryzen_smu" && make ) || fail "ryzen_smu 编译失败"
  cp "$TMP/ryzen_smu/ryzen_smu.ko" "$INSTALL_ROOT/modules/ryzen_smu.ko"
  rm -rf "$TMP"
  ROLLBACK_ACTIONS+=("rm -f $INSTALL_ROOT/modules/ryzen_smu.ko")
fi

if ! lsmod | grep -q '^ryzen_smu'; then
  insmod "$INSTALL_ROOT/modules/ryzen_smu.ko" 2>/dev/null \
    || log "警告：ryzen_smu 加载失败（内核版本不匹配？），TDP 控制不可用，遥测/语言/主题仍正常"
fi
log "ryzen_smu: $(lsmod | grep '^ryzen_smu' || echo 未加载)"

# ── Step 2 it87 风扇驱动 ──────────────────────────────────────────
step 2 "it87 风扇驱动（IT8620E Super-IO）"
install -m644 "$SRC_CFG/it87.conf" /etc/modprobe.d/it87.conf
install -m644 "$SRC_CFG/it87-load.conf" /etc/modules-load.d/it87.conf
ROLLBACK_ACTIONS+=("rm -f /etc/modprobe.d/it87.conf /etc/modules-load.d/it87.conf")
modprobe it87 force_id=0x8620 2>/dev/null || log "警告：it87 加载失败（风扇转速将为 0）"
log "it87: $(lsmod | grep '^it87' || echo 未加载)"

# ── Step 3 ryzenadj ───────────────────────────────────────────────
step 3 "ryzenadj（TDP 控制 CLI）"
if command -v ryzenadj >/dev/null 2>&1; then
  log "ryzenadj 已存在：$(command -v ryzenadj)"
else
  if [ -f "$SRC_BIN/ryzenadj" ]; then
    mkdir -p "$INSTALL_ROOT/bin"
    install -m755 "$SRC_BIN/ryzenadj" "$INSTALL_ROOT/bin/ryzenadj"
    ln -sf "$INSTALL_ROOT/bin/ryzenadj" /usr/local/bin/ryzenadj 2>/dev/null \
      || log "提示：/usr/local/bin 只读，ryzenadj 仅通过服务 PATH 暴露"
    ROLLBACK_ACTIONS+=("rm -f /usr/local/bin/ryzenadj $INSTALL_ROOT/bin/ryzenadj")
  else
    log "警告：未找到 ryzenadj，TDP 预设将报错。请手动安装 RyzenAdj。"
  fi
fi

# ── Step 4 部署后台服务 ───────────────────────────────────────────
step 4 "am02-service 后台服务"
mkdir -p "$INSTALL_ROOT"
install -m755 "$SRC_BIN/am02-service" "$INSTALL_ROOT/am02-service"
command -v chcon >/dev/null 2>&1 && chcon -t bin_t "$INSTALL_ROOT/am02-service" 2>/dev/null || true
install -m644 "$SRC_CFG/am02-service.service" /etc/systemd/system/am02-service.service
ROLLBACK_ACTIONS+=("systemctl stop am02-service || true; systemctl disable am02-service || true; rm -f /etc/systemd/system/am02-service.service; rm -rf $INSTALL_ROOT")

systemctl daemon-reload
systemctl enable --now am02-service || fail "am02-service 启动失败（详见 journalctl -u am02-service）"
log "am02-service: $(systemctl is-active am02-service)"

# ── Step 5 exfat 数据分区 ─────────────────────────────────────────
step 5 "exfat 数据分区"
mkdir -p /var/mnt/exfat
if ! grep -q 'UUID=3521-FB40' /etc/fstab 2>/dev/null; then
  printf 'UUID=3521-FB40 /var/mnt/exfat exfat defaults,nofail 0 0\n' >> /etc/fstab
  ROLLBACK_ACTIONS+=("sed -i '/UUID=3521-FB40/d' /etc/fstab")
fi
mount /var/mnt/exfat 2>/dev/null || log "警告：exfat 分区未挂载（nofail，不阻塞开机）"
log "exfat: $(mount | grep -c /var/mnt/exfat) 个挂载"

# ── Step 6 Decky Loader + 插件 ────────────────────────────────────
step 6 "Decky 插件"
PLUGIN_USER="$(resolve_plugin_user)"
PLUGIN_DST="/home/$PLUGIN_USER/homebrew/plugins/am02-decky"
if [ -z "$PLUGIN_USER" ] || [ ! -d "/home/$PLUGIN_USER/homebrew/plugins" ]; then
  fail "未检测到 Decky Loader。请先执行 ujust setup-decky install 后重跑本脚本。"
fi
rm -rf "$PLUGIN_DST"
cp -r "$SRC_PLUGIN" "$PLUGIN_DST" || fail "复制插件失败"
chown -R "$PLUGIN_USER":"$PLUGIN_USER" "$PLUGIN_DST"
# 兼容两种 Decky 服务形态：SteamOS 系统级 plugin_loader-release / Bazzite user 级 plugin_loader
if systemctl list-unit-files 2>/dev/null | grep -q '^plugin_loader-release'; then
  systemctl restart plugin_loader-release 2>/dev/null \
    || log "提示：无法热重启 plugin_loader-release，重启机器后插件生效"
else
  runuser -u "$PLUGIN_USER" -- systemctl --user restart plugin_loader 2>/dev/null \
    || log "提示：无法热重启 plugin_loader，重启机器后插件生效"
fi
log "插件已部署到 $PLUGIN_DST"

# ── Step 7 收尾校验 ───────────────────────────────────────────────
step 7 "收尾校验"
echo "  - 服务:   $(systemctl is-active am02-service)"
echo "  - 模块:   ryzen_smu=$(lsmod | grep -c '^ryzen_smu') it87=$(lsmod | grep -c '^it87')"
echo "  - 串口:   $([ -e /dev/ttyS0 ] && echo OK || echo MISSING)"
echo "  - 插件:   $([ -d "$PLUGIN_DST" ] && echo OK || echo MISSING)"
echo "  - exfat:  $(mount | grep -c /var/mnt/exfat) 个挂载"

echo
echo "[am02-setup] 安装完成。"
echo "  若插件未出现在快捷菜单（QAM），请重启机器。"
echo "  ⚠️ 重要：请务必完成环境适配（默认输出 eDP→HDMI），见 README 的「环境适配」章节。"
log "安装完成"
