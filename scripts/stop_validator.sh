#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.0 — STOP VALIDATOR DAEMON
# =============================================================================

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

echo "Stopping Hydra-HFT 48h Validator and Dashboard..."

if [ -f "logs/paper_48h.pid" ]; then
    PID=$(cat logs/paper_48h.pid 2>/dev/null || echo "")
    if [ -n "$PID" ]; then
        kill "$PID" 2>/dev/null || true
        echo " -> Stopped hydra-node (PID $PID)"
    fi
    rm -f logs/paper_48h.pid
fi

if [ -f "logs/web_dashboard.pid" ]; then
    PID=$(cat logs/web_dashboard.pid 2>/dev/null || echo "")
    if [ -n "$PID" ]; then
        kill "$PID" 2>/dev/null || true
        echo " -> Stopped web dashboard (PID $PID)"
    fi
    rm -f logs/web_dashboard.pid
fi

if [ -f "logs/git_sync.pid" ]; then
    PID=$(cat logs/git_sync.pid 2>/dev/null || echo "")
    if [ -n "$PID" ]; then
        kill "$PID" 2>/dev/null || true
        echo " -> Stopped git sync (PID $PID)"
    fi
    rm -f logs/git_sync.pid
fi

pkill -f "hydra-node --paper-48h" 2>/dev/null || true
pkill -f "web_dashboard.py" 2>/dev/null || true
pkill -f "sync_telemetry_git.sh" 2>/dev/null || true

echo "Hydra-HFT validator stopped."
