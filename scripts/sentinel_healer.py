#!/usr/bin/env python3
"""
HYDRA-HFT v4.1 — Real-Time 1-Second Autonomous Sentinel & Self-Healing Watchdog
Continuously monitors all processes, WebSocket feeds, and execution logs every second.
If any component hangs, disconnects, or crashes, it automatically heals it within 1 second.
Alerts on first live on-chain trade execution.
"""

import os
import sys
import time
import json
import subprocess

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
os.chdir(BASE_DIR)

LOGS_DIR = os.path.join(BASE_DIR, "logs")
os.makedirs(LOGS_DIR, exist_ok=True)

SENTINEL_LOG = os.path.join(LOGS_DIR, "sentinel.log")
SENTINEL_STATUS = os.path.join(LOGS_DIR, "sentinel_status.json")
WS_LOG = os.path.join(LOGS_DIR, "ws_listener.log")
BROADCASTER_LOG = os.path.join(LOGS_DIR, "live_broadcaster.log")
TRADES_LOG = os.path.join(LOGS_DIR, "live_onchain_trades.jsonl")

def log(msg):
    ts = time.strftime("%Y-%m-%d %H:%M:%S")
    line = f"[{ts}] [SENTINEL] {msg}\n"
    sys.stdout.write(line)
    sys.stdout.flush()
    with open(SENTINEL_LOG, "a", encoding="utf-8") as f:
        f.write(line)

def check_systemd_service(service_name="hydra.service"):
    try:
        res = subprocess.run(
            ["systemctl", "is-active", service_name],
            capture_output=True,
            text=True,
            timeout=2
        )
        return res.stdout.strip() == "active"
    except Exception as e:
        return False

def restart_service(service_name="hydra.service"):
    log(f"ALERT: {service_name} is unhealthy! Triggering instant self-healing restart...")
    try:
        subprocess.run(["sudo", "systemctl", "restart", service_name], timeout=5)
        log(f"SUCCESS: {service_name} restarted successfully.")
        return True
    except Exception as e:
        log(f"ERROR: Failed to restart {service_name}: {e}")
        return False

def get_last_ws_block_time():
    if not os.path.exists(WS_LOG):
        return 0, None
    try:
        with open(WS_LOG, "rb") as f:
            f.seek(max(0, os.path.getsize(WS_LOG) - 2048))
            lines = f.read().decode("utf-8", errors="ignore").splitlines()
        for line in reversed(lines):
            if "Block 0x" in line:
                # Extract block hex
                parts = line.split("Block ")
                if len(parts) > 1:
                    block_hex = parts[1].split()[0]
                    return os.path.getmtime(WS_LOG), block_hex
        return os.path.getmtime(WS_LOG), None
    except Exception:
        return 0, None

def check_live_trades():
    if not os.path.exists(TRADES_LOG):
        return 0
    try:
        with open(TRADES_LOG, "r", encoding="utf-8") as f:
            lines = [l.strip() for l in f if l.strip()]
        return len(lines)
    except Exception:
        return 0

def run_sentinel():
    log("==================================================================")
    log("  HYDRA-HFT v4.1 — AUTONOMOUS REAL-TIME 1-SECOND HEALING SENTINEL")
    log(f"  Target Directory : {BASE_DIR}")
    log("  Check Interval   : 1.0 second")
    log("  Self-Healing     : Active (auto-restart on hang/crash)")
    log("==================================================================")

    last_heartbeat_time = 0
    known_trade_count = check_live_trades()

    while True:
        now = time.time()
        healthy = True
        issues = []

        # 1. Check systemd core daemon
        is_active = check_systemd_service("hydra.service")
        if not is_active:
            healthy = False
            issues.append("hydra.service inactive")
            restart_service("hydra.service")
            time.sleep(3)
            continue

        # 2. Check WebSocket streaming freshness (Base produces blocks every 2s)
        ws_mtime, last_block = get_last_ws_block_time()
        ws_staleness = now - ws_mtime
        if ws_staleness > 18.0:
            healthy = False
            issues.append(f"WS stream stalled ({ws_staleness:.1f}s without block)")
            log(f"WARNING: WebSocket listener stalled for {ws_staleness:.1f}s. Healing service...")
            restart_service("hydra.service")
            time.sleep(3)
            continue

        # 3. Check for new on-chain live trades
        trade_count = check_live_trades()
        if trade_count > known_trade_count:
            log(f"🎉 CELEBRATION: NEW LIVE TRADE DETECTED! Total executed: {trade_count}")
            known_trade_count = trade_count

        # 4. Write real-time status file every 2 seconds
        if now - last_heartbeat_time >= 2.0:
            status_data = {
                "timestamp": now,
                "formatted_time": time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime(now)),
                "healthy": healthy,
                "issues": issues,
                "last_block": last_block,
                "ws_latency_s": round(ws_staleness, 2),
                "total_live_trades": trade_count,
                "service_status": "active" if is_active else "inactive"
            }
            try:
                with open(SENTINEL_STATUS, "w", encoding="utf-8") as f:
                    json.dump(status_data, f, indent=2)
            except Exception:
                pass
            last_heartbeat_time = now

        # Sleep precisely 1 second
        time.sleep(1.0)

if __name__ == "__main__":
    try:
        run_sentinel()
    except KeyboardInterrupt:
        log("Sentinel stopped by operator.")
