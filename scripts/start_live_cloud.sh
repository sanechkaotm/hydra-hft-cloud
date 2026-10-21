#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.1 — MASTER LIVE CLOUD LAUNCHER (AWS EC2)
# Starts the Live High-Frequency Trading Engine with On-Chain Broadcaster,
# EVM Simulation Pre-Flight Protection, and Telegram Alerting.
# =============================================================================
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

mkdir -p logs

echo "=========================================================================="
echo " HYDRA-HFT v4.1 — LAUNCHING LIVE TRADING ON BASE MAINNET (L2: 8453)"
echo " Server Location : AWS EC2 (us-east-1)"
echo " Contract        : 0x2d1a594e982ee19d2718992991d3835bfec5d4ac"
echo "=========================================================================="

# 1. Check .env configuration
if [ ! -f ".env" ]; then
    echo "[!] Error: .env file missing in $REPO_DIR!"
    echo "    Please create .env with HYDRA_HOT_WALLET_PRIVKEY and HYDRA_WALLET_ADDRESS."
    exit 1
fi

# 2. Stop old stale background instances
echo "[1/5] Stopping any existing Hydra processes..."
pkill -f "hydra-node" 2>/dev/null || true
pkill -f "live_broadcaster.py" 2>/dev/null || true
pkill -f "telegram_bot.py" 2>/dev/null || true
pkill -f "web_dashboard.py" 2>/dev/null || true
sleep 1

# 3. Ensure python dependencies
echo "[2/5] Checking python dependencies..."
pip install -q eth-account python-dotenv web3 2>/dev/null || pip3 install -q eth-account python-dotenv web3 2>/dev/null || true

# 4. Compile release binary if needed
echo "[3/5] Verifying Rust HFT release binary..."
if [ ! -f "target/release/hydra-node" ] || [ -n "$(git status --porcelain crates/)" ]; then
    echo " Compiling zero-allocation release engine..."
    cargo build --release
fi

# 5. Start live broadcaster (Anomaly Guardian & EVM Pre-flight Simulation)
echo "[4/5] Starting Live Transaction Broadcaster on UDP 127.0.0.1:8765..."
nohup python3 scripts/live_broadcaster.py > logs/live_broadcaster.log 2>&1 &
echo $! > logs/live_broadcaster.pid
echo " -> Broadcaster active (PID: $(cat logs/live_broadcaster.pid))"

# 6. Start Telegram Bot
echo " Starting Telegram alert bot..."
nohup python3 scripts/telegram_bot.py > logs/telegram_bot.log 2>&1 &
echo $! > logs/telegram_bot.pid

# 7. Start Web Dashboard on port 8080
echo " Starting Web Dashboard on port 8080..."
nohup python3 scripts/web_dashboard.py 8080 > logs/web_dashboard.log 2>&1 &
echo $! > logs/web_dashboard.pid

# 8. Start Hydra-Node in Live mode
echo "[5/5] Launching Hydra HFT Live Engine..."
nohup ./target/release/hydra-node --paper-48h --live --capital 15.00 --polls 0 > logs/hydra_live_stdout.log 2>&1 &
echo $! > logs/hydra_live.pid
echo " -> Hydra Node active (PID: $(cat logs/hydra_live.pid))"

echo "=========================================================================="
echo " [OK] HYDRA-HFT v4.1 LIVE MODE IS RUNNING IN BACKGROUND!"
echo " Logs: tail -f logs/live_broadcaster.log"
echo " Dashboard: http://54.87.135.149:8080"
echo "=========================================================================="
