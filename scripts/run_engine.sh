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
pip install -q eth-account python-dotenv web3 2>/dev/null || pip3 install -q eth-account python-dotenv web3 2>/dev/null || true

# Determine execution mode
if [ -n "${HYDRA_HOT_WALLET_PRIVKEY:-}" ] && [ -n "${HYDRA_EXECUTOR_CONTRACT:-}" ]; then
    echo "[RUNNER] Live credentials detected! Starting in LIVE ON-CHAIN MODE..."
    
    # 1. Start Anomaly Guardian Broadcaster (UDP 8765) if not running
    if ! pgrep -f "live_broadcaster.py" > /dev/null 2>&1; then
        echo "[RUNNER] Launching Live Broadcaster & EVM Preflight Guardian..."
        python3 scripts/live_broadcaster.py > logs/live_broadcaster.log 2>&1 &
    fi

    # 2. Start Telegram alert bot if not running
    if ! pgrep -f "telegram_bot.py" > /dev/null 2>&1; then
        echo "[RUNNER] Launching Telegram Alert Bot..."
        python3 scripts/telegram_bot.py > logs/telegram_bot.log 2>&1 &
    fi

    CAPITAL="${HYDRA_INITIAL_CAPITAL_EUR:-15.00}"
    echo "[RUNNER] Executing hydra-node --paper-48h --live --capital $CAPITAL..."
    exec ./target/release/hydra-node --paper-48h --live --capital "$CAPITAL" --polls 0
else
    echo "[RUNNER] No live keys in .env. Running in 48-Hour Paper Validation mode..."
    exec ./target/release/hydra-node --paper-48h --capital 150.00 --polls 0
fi
