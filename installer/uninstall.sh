#!/usr/bin/env bash
# =============================================================================
#  AYANEO AM02 副屏复刻 —— 卸载脚本
#  用法: sudo bash uninstall.sh
#  说明: 停服务 → 删文件 → 卸模块 → 还原 fstab → 删插件（Decky Loader 本体不卸载）
# =============================================================================
set -u
INSTALL_ROOT="/var/lib/am02-service"

[ "$(id -u)" -eq 0 ] || { echo "请用 sudo 运行"; exit 1; }

echo "[am02-uninstall] 开始卸载 AYANEO AM02 副屏复刻..."

# 1. 停服务 + 删 unit
systemctl stop am02-service 2>/dev/null || true
systemctl disable am02-service 2>/dev/null || true
rm -f /etc/systemd/system/am02-service.service
systemctl daemon-reload

# 2. 卸载内核模块
rmmod it87 2>/dev/null || true
rmmod ryzen_smu 2>/dev/null || true

# 3. 删 it87 配置
rm -f /etc/modprobe.d/it87.conf /etc/modules-load.d/it87.conf

# 4. 还原 fstab（只删本工具写入的 exfat 行）
if grep -q 'UUID=3521-FB40' /etc/fstab 2>/dev/null; then
  sed -i '/UUID=3521-FB40/d' /etc/fstab
  umount /var/mnt/exfat 2>/dev/null || true
fi

# 5. 删二进制 / 模块
rm -rf "$INSTALL_ROOT"
rm -f /usr/local/bin/ryzenadj

# 6. 删 Decky 插件
PLUGIN_DST="/home/${SUDO_USER:-deck}/homebrew/plugins/am02-decky"
if [ -d "$PLUGIN_DST" ]; then
  rm -rf "$PLUGIN_DST"
  echo "  已删除插件 $PLUGIN_DST"
fi

echo
echo "[am02-uninstall] 卸载完成。"
echo "  - Decky Loader 本体未卸载（如需：ujust setup-decky uninstall）"
echo "  - ryzenadj（若为系统自带）未卸载"
