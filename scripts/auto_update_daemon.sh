#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.0 — Continuous GitOps Auto-Updater Daemon
# Checks GitHub origin/main every 60 seconds.
# When a new commit is detected:
#   1. Pulls the latest code
#   2. Recompiles the release binary (if Rust files changed)
#   3. Gracefully restarts hydra.service with the updated code/settings
# This allows changing server settings via GitHub without ever touching AWS!
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
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] New commit detected ($LOCAL -> $REMOTE). Applying update..."
        
        # Check if Rust source files changed
        DIFF_SRC=$(git diff --name-only HEAD origin/main | grep -E '\.rs$|Cargo\.' || true)
        
        # Fast-forward to latest commit
        git reset --hard origin/main
        chmod +x scripts/*.sh 2>/dev/null || true

        if [ -n "$DIFF_SRC" ]; then
            echo "Rust source changes detected. Recompiling release binary..."
            export PATH="$HOME/.cargo/bin:$PATH"
            cargo build --release
        fi

        echo "Restarting hydra.service with new updates..."
        sudo systemctl restart hydra.service 2>/dev/null || true
        echo "Update successfully deployed!"
    fi
done
