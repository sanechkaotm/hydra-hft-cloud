#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.0 — SYSTEMD SERVICE INSTALLER ("ONCE AND FOR ALL")
# Configures Hydra-HFT as a permanent OS-level background service that:
#   1. Starts automatically on server reboot
#   2. Automatically restarts in 5 seconds if it ever stops or crashes
#   3. Runs completely unattended 24/7/365
# =============================================================================
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CURRENT_USER="${SUDO_USER:-$(id -un)}"
USER_HOME=$(eval echo "~$CURRENT_USER")

echo "=========================================================================="
echo " HYDRA-HFT v4.0 — INSTALLING PERMANENT SYSTEMD SERVICE (24/7/365)"
echo " Repository Directory : $REPO_DIR"
echo " Service User         : $CURRENT_USER ($USER_HOME)"
echo "=========================================================================="

# 1. Compile release binary if missing
if [ ! -f "$REPO_DIR/target/release/hydra-node" ]; then
    echo "Compiling release binary..."
    cd "$REPO_DIR"
    export PATH="$USER_HOME/.cargo/bin:$PATH"
    cargo build --release
fi

mkdir -p "$REPO_DIR/logs"
chown -R "$CURRENT_USER:$CURRENT_USER" "$REPO_DIR" 2>/dev/null || true

# 2. Write Systemd Unit File
SERVICE_FILE="/etc/systemd/system/hydra.service"
echo "Writing $SERVICE_FILE..."

sudo bash -c "cat <<EOF > $SERVICE_FILE
[Unit]
Description=Hydra-HFT v4.0 Distributed Quantitative Arbitrage Engine
After=network.target network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$CURRENT_USER
WorkingDirectory=$REPO_DIR
Environment=PATH=$USER_HOME/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
ExecStart=/bin/bash $REPO_DIR/scripts/run_engine.sh
Restart=always
RestartSec=5
StandardOutput=append:$REPO_DIR/logs/paper_48h_stdout.log
StandardError=append:$REPO_DIR/logs/paper_48h_stdout.log
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF"

# 3. Write Web Dashboard Service (optional UI on port 8080)
DASHBOARD_SERVICE_FILE="/etc/systemd/system/hydra-dashboard.service"
echo "Writing $DASHBOARD_SERVICE_FILE..."

sudo bash -c "cat <<EOF > $DASHBOARD_SERVICE_FILE
[Unit]
Description=Hydra-HFT Live Web Telemetry Dashboard (Port 8080)
After=hydra.service

[Service]
Type=simple
User=$CURRENT_USER
WorkingDirectory=$REPO_DIR
ExecStart=/usr/bin/python3 $REPO_DIR/scripts/web_dashboard.py 8080
Restart=always
RestartSec=10
StandardOutput=append:$REPO_DIR/logs/web_dashboard.log
StandardError=append:$REPO_DIR/logs/web_dashboard.log

[Install]
WantedBy=multi-user.target
EOF"

# 4. Write GitOps Auto-Updater Service (monitors GitHub for changes)
UPDATER_SERVICE_FILE="/etc/systemd/system/hydra-updater.service"
echo "Writing $UPDATER_SERVICE_FILE..."

sudo bash -c "cat <<EOF > $UPDATER_SERVICE_FILE
[Unit]
Description=Hydra-HFT Continuous GitOps Auto-Updater Service
After=network.target

[Service]
Type=simple
User=$CURRENT_USER
WorkingDirectory=$REPO_DIR
ExecStart=/bin/bash $REPO_DIR/scripts/auto_update_daemon.sh
Restart=always
RestartSec=15
StandardOutput=append:$REPO_DIR/logs/git_updater.log
StandardError=append:$REPO_DIR/logs/git_updater.log

[Install]
WantedBy=multi-user.target
EOF"

# 5. Reload and Enable systemd
echo "Reloading systemd and enabling permanent auto-start on boot..."
sudo systemctl daemon-reload
sudo systemctl enable hydra.service
sudo systemctl enable hydra-dashboard.service
sudo systemctl enable hydra-updater.service

echo "Starting services now..."
sudo systemctl restart hydra.service
sudo systemctl restart hydra-dashboard.service
sudo systemctl restart hydra-updater.service

echo ""
echo "=========================================================================="
echo " [SUCCESS] HYDRA-HFT IS NOW INSTALLED AS A PERMANENT SYSTEM SERVICE!"
echo "=========================================================================="
echo " Service Status  : sudo systemctl status hydra.service"
echo " Restart Service : sudo systemctl restart hydra.service"
echo " Stop Service    : sudo systemctl stop hydra.service"
echo " Logs Live Stream: journalctl -u hydra.service -f (or tail -f logs/paper_48h_stdout.log)"
echo ""
echo " -> The bot will now run FOREVER, even after server reboots or crashes."
echo "=========================================================================="
