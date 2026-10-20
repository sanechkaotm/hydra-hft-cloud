#!/usr/bin/env bash
set -euo pipefail

echo "================================================================="
echo " HYDRA-HFT v4.0 — AUTOMATED CLOUD SERVER DEPLOYMENT & SETUP"
echo " Target Instance: AWS EC2 (t3.medium) | Branch: main"
echo "================================================================="

# 1. Update OS and install dependencies
echo "[1/5] Updating apt packages and installing system tools..."
sudo apt-get update -y
sudo apt-get install -y build-essential curl git screen pkg-config libssl-dev

# 2. Install Rust Toolchain if not already present
echo "[2/5] Checking Rust compiler toolchain..."
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust via rustup (2024 edition)..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "Rust is already installed: $(rustc --version)"
fi

# Ensure cargo is in PATH for current subshell
export PATH="$HOME/.cargo/bin:$PATH"

# 3. Clone or pull latest codebase from GitHub
echo "[3/5] Syncing Hydra-HFT monorepo from GitHub..."
REPO_DIR="$HOME/hydra-hft-cloud"
if [ -d "$REPO_DIR" ]; then
    echo "Existing repository found at $REPO_DIR, pulling latest origin/main..."
    cd "$REPO_DIR"
    git fetch origin main
    git reset --hard origin/main
else
    echo "Cloning repository to $REPO_DIR..."
    git clone https://github.com/sanechkaotm/hydra-hft-cloud.git "$REPO_DIR"
    cd "$REPO_DIR"
fi

# 4. Compile optimized release binary
echo "[4/5] Compiling Hydra-HFT release binary with fat LTO and opt-level 3..."
cargo build --release

# 5. Run Pre-flight sanity check on the server
echo "[5/5] Running pre-flight diagnostics directly from the data-center..."
./target/release/hydra-node --preflight-check --capital 20.0

echo ""
echo "================================================================="
echo " [SUCCESS] HYDRA-HFT IS FULLY COMPILED AND READY ON THIS SERVER!"
echo " Directory: $REPO_DIR"
echo " Binary:    $REPO_DIR/target/release/hydra-node"
echo ""
echo " To run in 24/7 background mode (Screen):"
echo "   screen -S hydra"
echo "   cd $REPO_DIR"
echo "   ./target/release/hydra-node --live-market --capital 20.0"
echo "   (Press Ctrl+A then D to detach)"
echo "================================================================="
