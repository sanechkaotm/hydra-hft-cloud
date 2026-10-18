#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.0 — UBUNTU 24.04 LTS LOW-LATENCY HOST TUNING SCRIPT
# Target Nodes: Tokyo (ap-northeast-1), London (eu-west-2), Frankfurt (eu-central-1)
# =============================================================================
set -euo pipefail

echo "[HYDRA-HFT] Applying kernel sysctl low-latency parameters..."
sudo cp "$(dirname "$0")/sysctl-hft.conf" /etc/sysctl.d/99-hydra-hft.conf
sudo modprobe tcp_bbr || true
sudo sysctl --system

echo "[HYDRA-HFT] Locking CPU frequency scaling governor to 'performance'..."
for gov in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
    if [ -f "$gov" ]; then
        echo performance | sudo tee "$gov" > /dev/null
    fi
done

echo "[HYDRA-HFT] Disabling Transparent Huge Pages (THP) to eliminate compaction jitter..."
if [ -f /sys/kernel/mm/transparent_hugepage/enabled ]; then
    echo never | sudo tee /sys/kernel/mm/transparent_hugepage/enabled > /dev/null
    echo never | sudo tee /sys/kernel/mm/transparent_hugepage/defrag > /dev/null
fi

PRIMARY_IFACE=$(ip route show default | awk '/default/ {print $5; exit}')
if [ -n "${PRIMARY_IFACE:-}" ] && command -v ethtool > /dev/null 2>&1; then
    echo "[HYDRA-HFT] Disabling NIC interrupt coalescing and GRO/LRO on ${PRIMARY_IFACE}..."
    sudo ethtool -K "$PRIMARY_IFACE" gro off lro off tso off gso off || true
    sudo ethtool -C "$PRIMARY_IFACE" rx-usecs 0 tx-usecs 0 || true
fi

echo "[HYDRA-HFT] Host tuning complete."
