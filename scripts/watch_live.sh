#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.0 — Terminal Live Telemetry Watcher
# Runs inside the AWS Session Manager terminal to stream real-time metrics
# =============================================================================

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

clear
echo "=========================================================================="
echo " HYDRA-HFT v4.0 — LIVE 48-HOUR TELEMETRY WATCHER (AWS EC2 us-east-1)"
echo " Press Ctrl+C at any time to exit the watcher (daemon keeps running)"
echo "=========================================================================="
echo ""

if [ -f "logs/paper_48h.pid" ]; then
    PID=$(cat logs/paper_48h.pid 2>/dev/null || echo "")
    if ps -p "$PID" > /dev/null 2>&1; then
        echo -e " \033[1;32m[● DAEMON RUNNING]\033[0m PID: $PID"
    else
        echo -e " \033[1;33m[○ DAEMON STOPPED]\033[0m Stale PID: $PID"
    fi
else
    echo -e " \033[1;33m[○ DAEMON STATUS UNKNOWN]\033[0m"
fi

echo ""
echo "Streaming latest status from logs/paper_48h_live_status.md..."
echo "--------------------------------------------------------------------------"

while true; do
    if [ -f "logs/paper_48h_live_status.md" ]; then
        # Print clean status
        cat logs/paper_48h_live_status.md
        echo "--------------------------------------------------------------------------"
        echo "Latest 3 ticks from CSV:"
        tail -n 3 logs/paper_48h_150eur.csv 2>/dev/null || echo "(no csv entries yet)"
        echo "--------------------------------------------------------------------------"
        echo "[Last refreshed: $(date '+%Y-%m-%d %H:%M:%S UTC')] (Refreshing in 3s...)"
    else
        echo "Waiting for validator to write first telemetry poll..."
    fi
    sleep 3
    # Move cursor back up or clear screen
    clear
done
