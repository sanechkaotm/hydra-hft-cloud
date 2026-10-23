#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.1 — UNIFIED ENGINE RUNNER (SYSTEMD & DAEMON ENTRYPOINT)
# Automatically detects if LIVE credentials exist in .env:
# - If HYDRA_HOT_WALLET_PRIVKEY is set -> Runs LIVE trading on Base L2 with
#   Anomaly Guardian, EVM Preflight, and Telegram alerts.
# - If not set -> Runs 48h Paper Validation mode safely.
# =============================================================================
set -e

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

mkdir -p logs

# Load .env if present
if [ -f ".env" ]; then
    set -a
    source .env 2>/dev/null || true
    set +a
fi

# Ensure python requirements
pip install -q eth-account python-dotenv web3 requests websockets 2>/dev/null || pip3 install -q eth-account python-dotenv web3 requests websockets 2>/dev/null || true

# Determine execution mode
if [ -n "${HYDRA_HOT_WALLET_PRIVKEY:-}" ] && [ -n "${HYDRA_EXECUTOR_CONTRACT:-}" ]; then
    echo "[RUNNER] Live credentials detected! Starting in LIVE ON-CHAIN MODE..."
    
    # 1. Start Anomaly Guardian Broadcaster (UDP 8765) if not running
    if ! pgrep -f "live_broadcaster.py" > /dev/null 2>&1; then
        echo "[RUNNER] Launching Live Broadcaster & EVM Preflight Guardian..."
        python3 scripts/live_broadcaster.py > logs/live_broadcaster.log 2>&1 &
    fi

    # 2. Start WebSocket Event Streamer (UDP 8766 reactive wake-up) if not running
    if ! pgrep -f "ws_listener.py" > /dev/null 2>&1; then
        echo "[RUNNER] Launching Real-Time Base WebSocket Event Streamer..."
        python3 scripts/ws_listener.py > logs/ws_listener.log 2>&1 &
    fi

    # 3. Start Telegram alert bot if not running
    if ! pgrep -f "telegram_bot.py" > /dev/null 2>&1; then
        echo "[RUNNER] Launching Telegram Alert Bot..."
        python3 scripts/telegram_bot.py > logs/telegram_bot.log 2>&1 &
    fi

    CAPITAL="${HYDRA_INITIAL_CAPITAL_EUR:-15.00}"
    echo "[RUNNER] Executing hydra-node --paper-48h --live --capital $CAPITAL..."
    MIN_DEX_PROFIT="${HYDRA_MIN_DEX_PROFIT_EUR:-0.20}"
    MIN_BPS="${HYDRA_MIN_NET_PROFIT_BPS:-10.0}"
    echo "[RUNNER] Executing hydra-node --paper-48h --live --capital $CAPITAL --min-dex-profit $MIN_DEX_PROFIT --min-bps $MIN_BPS --polls 0..."
    exec ./target/release/hydra-node --paper-48h --live --capital "$CAPITAL" --min-dex-profit "$MIN_DEX_PROFIT" --min-bps "$MIN_BPS" --polls 0
else
    echo "[RUNNER] No live keys in .env. Running in 48-Hour Paper Validation mode..."
    exec ./target/release/hydra-node --paper-48h --capital 150.00 --polls 0
fi
