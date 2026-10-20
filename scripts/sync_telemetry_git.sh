#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.0 — Background Telemetry Git Sync Daemon
# Periodically pushes logs to GitHub every 15 minutes
# =============================================================================

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

while true; do
    sleep 900 # 15 minutes
    if [ -f "logs/paper_48h_live_status.md" ]; then
        git config user.name "Hydra-HFT Cloud Node" || true
        git config user.email "cloud-node@hydra-hft.local" || true
        git add logs/paper_48h_live_status.md logs/paper_48h_150eur.csv 2>/dev/null || true
        git commit -m "Auto-sync live €150 paper telemetry [skip ci]" 2>/dev/null || true
        git push origin main 2>/dev/null || true
    fi
done
