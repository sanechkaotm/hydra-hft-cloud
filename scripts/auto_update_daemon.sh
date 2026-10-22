#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.0 — Continuous GitOps Auto-Updater Daemon
# Checks GitHub origin/main every 60 seconds.
# When a new commit is detected:
#   1. Pulls the latest code
#   2. Recompiles the release binary (if Rust files changed)
#   3. Gracefully restarts hydra.service ONLY if real code changed (crates/, scripts/)
#   4. If only logs/telemetry changed, fast-forwards WITHOUT restarting to keep 100% uptime
# =============================================================================

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

echo "Hydra GitOps Auto-Updater active for $REPO_DIR (monitoring origin/main)..."

while true; do
    sleep 60
    # Fetch latest remote references quietly
    git fetch origin main >/dev/null 2>&1 || true
    LOCAL=$(git rev-parse HEAD 2>/dev/null || echo "")
    REMOTE=$(git rev-parse origin/main 2>/dev/null || echo "")

    if [ -n "$REMOTE" ] && [ -n "$LOCAL" ] && [ "$LOCAL" != "$REMOTE" ]; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] New commit detected ($LOCAL -> $REMOTE). Inspecting diff..."
        
        # Check if real executable code / scripts changed (excluding telemetry logs, csv, markdown, github workflows)
        DIFF_CODE=$(git diff --name-only HEAD origin/main | grep -v -E '^logs/|\.md$|\.txt$|\.csv$|^\.github/' || true)
        DIFF_SRC=$(git diff --name-only HEAD origin/main | grep -E '\.rs$|Cargo\.' || true)
        
        # Fast-forward to latest commit quietly
        git reset --hard origin/main
        chmod +x scripts/*.sh 2>/dev/null || true

        if [ -z "$DIFF_CODE" ]; then
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Only telemetry/docs changed. Skipping service restart to maintain 100% engine uptime."
            continue
        fi

        if [ -n "$DIFF_SRC" ]; then
            echo "Rust source changes detected. Recompiling release binary..."
            export PATH="$HOME/.cargo/bin:$PATH"
            cargo build --release
        fi

        # Ensure systemd service points to unified run_engine.sh
        if [ -f "/etc/systemd/system/hydra.service" ] && grep -q "target/release/hydra-node" /etc/systemd/system/hydra.service 2>/dev/null; then
            echo "Updating hydra.service to use unified run_engine.sh..."
            sudo sed -i "s|ExecStart=.*|ExecStart=/bin/bash $REPO_DIR/scripts/run_engine.sh|" /etc/systemd/system/hydra.service 2>/dev/null || true
            sudo systemctl daemon-reload 2>/dev/null || true
        fi

        echo "Restarting hydra.service with new code updates..."
        sudo systemctl restart hydra.service 2>/dev/null || true
        echo "Code update successfully deployed!"
    fi
done
