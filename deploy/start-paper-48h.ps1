# =============================================================================
# HYDRA-HFT v4.0 — WINDOWS 48-HOUR €150.00 LIVE MARKET PAPER-TRADING LAUNCHER
# Starts hydra-node.exe --paper-48h --capital 150 --polls 0 in the background
# and logs every live tick to:
#   - logs/paper_48h_150eur.csv
#   - logs/paper_48h_live_status.md
# =============================================================================
$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Force -Path "logs" | Out-Null
Write-Host "Starting Hydra-HFT v4.0 48-Hour Live Market Validator (€150.00 Starting Equity)..." -ForegroundColor Cyan

$proc = Start-Process -FilePath ".\target\release\hydra-node.exe" `
    -ArgumentList "--paper-48h", "--capital", "150.00", "--polls", "0" `
    -RedirectStandardOutput "logs\paper_48h_stdout.log" `
    -RedirectStandardError "logs\paper_48h_stderr.log" `
    -WindowStyle Hidden `
    -PassThru

$proc.Id | Out-File -FilePath "logs\paper_48h.pid" -Encoding ascii
Write-Host " -> Daemon running in background (PID: $($proc.Id))" -ForegroundColor Green
Write-Host " -> Check live dashboard : Get-Content logs\paper_48h_live_status.md"
Write-Host " -> Tail live CSV ticks  : Get-Content logs\paper_48h_150eur.csv -Tail 20 -Wait"
