#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.0 — MASTER 48-HOUR LIVE CLOUD VALIDATOR & MONITOR LAUNCHER
# Starts the continuous background validator + web telemetry dashboard
# =============================================================================
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

mkdir -p logs

echo "=========================================================================="
echo " HYDRA-HFT v4.0 — STARTING 48-HOUR LIVE MARKET PAPER VALIDATION"
echo " Starting Capital: €150.00 | Location: AWS EC2 (us-east-1)"
echo "=========================================================================="

# 1. Stop any stale background instances
echo "[1/4] Checking and stopping any stale background instances..."
if [ -f "logs/paper_48h.pid" ]; then
    OLD_PID=$(cat logs/paper_48h.pid 2>/dev/null || echo "")
    if [ -n "$OLD_PID" ] && ps -p "$OLD_PID" > /dev/null 2>&1; then
        echo " Stopping previous hydra-node process (PID $OLD_PID)..."
        kill -9 "$OLD_PID" 2>/dev/null || true
    fi
    rm -f logs/paper_48h.pid
fi

pkill -f "web_dashboard.py" 2>/dev/null || true
pkill -f "sync_telemetry_git.sh" 2>/dev/null || true

# 2. Build or verify binary
echo "[2/4] Verifying release binary..."
if [ ! -f "target/release/hydra-node" ]; then
    echo " Compiling release binary (fat LTO)..."
    cargo build --release
fi

# 3. Launch Hydra-Node Daemon in background
echo "[3/4] Launching Hydra-HFT 48h Validator in background (nohup)..."
nohup ./target/release/hydra-node --paper-48h --capital 150.00 --polls 0 > logs/paper_48h_stdout.log 2>&1 &
DAEMON_PID=$!
echo "$DAEMON_PID" > logs/paper_48h.pid
echo " -> Hydra-Node Daemon running (PID: $DAEMON_PID)"

# 4. Launch Live Web Dashboard on port 8080
echo "[4/5] Launching Web Telemetry Dashboard on port 8080..."
nohup python3 scripts/web_dashboard.py 8080 > logs/web_dashboard.log 2>&1 &
DASHBOARD_PID=$!
echo "$DASHBOARD_PID" > logs/web_dashboard.pid
echo " -> Web Dashboard running (PID: $DASHBOARD_PID)"

# 5. Launch GitHub Telemetry Auto-Sync Daemon (every 15 min)
echo "[5/5] Launching GitHub Auto-Sync daemon in background..."
nohup bash scripts/sync_telemetry_git.sh > logs/git_sync.log 2>&1 &
SYNC_PID=$!
echo "$SYNC_PID" > logs/git_sync.pid
echo " -> Git Auto-Sync running (PID: $SYNC_PID)"

echo ""
echo "=========================================================================="
echo " [SUCCESS] 48-HOUR LIVE MARKET VALIDATOR IS RUNNING IN BACKGROUND!"
echo "=========================================================================="
echo ""
echo " 3 WAYS TO TRACK LIVE TELEMETRY:"
echo ""
echo " 1. Live Terminal Watcher (inside this AWS terminal):"
echo "    ./scripts/watch_live.sh"
echo "    (or: tail -f logs/paper_48h_stdout.log)"
echo ""
echo " 2. Web Browser Dashboard (open from PC/phone):"
echo "    http://54.87.135.149:8080"
echo "    (Requires Inbound TCP port 8080 allowed in EC2 Security Group)"
echo ""
echo " 3. GitHub Telemetry Dashboard (always viewable online):"
echo "    https://github.com/sanechkaotm/hydra-hft-cloud/blob/main/logs/paper_48h_live_status.md"
echo ""
echo " To stop the validator at any time:"
echo "    ./scripts/stop_validator.sh"
echo "=========================================================================="
