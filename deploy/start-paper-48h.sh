#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.0 — 48-HOUR €150.00 LIVE MARKET PAPER-TRADING DAEMON LAUNCHER
# Runs continuously in the background for 48 hours (34,560 polls @ 5s interval)
# and logs every live tick & equity update to:
#   - logs/paper_48h_150eur.csv
#   - logs/paper_48h_live_status.md
# =============================================================================
set -euo pipefail

mkdir -p logs
echo "Starting Hydra-HFT v4.0 48-Hour Live Market Validator (€150.00 Starting Equity)..."
nohup ./target/release/hydra-node --paper-48h --capital 150.00 --polls 0 > logs/paper_48h_stdout.log 2>&1 &
DAEMON_PID=$!
echo "${DAEMON_PID}" > logs/paper_48h.pid
echo " -> Daemon started (PID: ${DAEMON_PID})"
echo " -> Inspect live PnL dashboard: cat logs/paper_48h_live_status.md"
echo " -> Tail live tick stream     : tail -f logs/paper_48h_stdout.log"
