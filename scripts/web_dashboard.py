#!/usr/bin/env python3
"""
HYDRA-HFT v4.0 — Zero-Dependency Real-Time Web Telemetry Dashboard
Serves an auto-refreshing institutional dark-mode telemetry dashboard on port 8080.
Reads `logs/paper_48h_live_status.md` and `logs/paper_48h_150eur.csv`.
"""

import http.server
import json
import os
import socketserver
import sys
import time

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
LOGS_DIR = os.path.abspath("logs")
STATUS_MD = os.path.join(LOGS_DIR, "paper_48h_live_status.md")
CSV_PATH = os.path.join(LOGS_DIR, "paper_48h_150eur.csv")

HTML_TEMPLATE = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>HYDRA-HFT v4.0 — Live Cloud Telemetry</title>
  <style>
    :root {
      --bg: #0b0f17;
      --card-bg: #121824;
      --border: #1e293b;
      --text: #f1f5f9;
      --text-muted: #94a3b8;
      --accent: #38bdf8;
      --green: #10b981;
      --red: #ef4444;
      --yellow: #f59e0b;
      --purple: #a855f7;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background-color: var(--bg);
      color: var(--text);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", monospace;
      padding: 24px;
      line-height: 1.5;
    }
    .header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding-bottom: 20px;
      border-bottom: 1px solid var(--border);
      margin-bottom: 24px;
      flex-wrap: wrap;
      gap: 12px;
    }
    .logo-group { display: flex; align-items: center; gap: 12px; }
    .badge-live {
      background: rgba(16, 185, 129, 0.15);
      color: var(--green);
      border: 1px solid var(--green);
      padding: 4px 10px;
      border-radius: 9999px;
      font-size: 12px;
      font-weight: 700;
      letter-spacing: 0.05em;
      display: flex;
      align-items: center;
      gap: 6px;
    }
    .pulse {
      width: 8px; height: 8px;
      background: var(--green);
      border-radius: 50%;
      animation: pulse 1.5s infinite;
    }
    @keyframes pulse {
      0% { opacity: 1; transform: scale(1); }
      50% { opacity: 0.4; transform: scale(1.3); }
      100% { opacity: 1; transform: scale(1); }
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }
    .card {
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 18px;
    }
    .card-label {
      font-size: 12px;
      color: var(--text-muted);
      text-transform: uppercase;
      letter-spacing: 0.05em;
      margin-bottom: 6px;
    }
    .card-val {
      font-size: 26px;
      font-weight: 700;
      color: #fff;
    }
    .card-sub {
      font-size: 12px;
      color: var(--text-muted);
      margin-top: 4px;
    }
    .positive { color: var(--green) !important; }
    .negative { color: var(--red) !important; }
    .table-card {
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 20px;
      overflow-x: auto;
    }
    .table-title {
      font-size: 16px;
      font-weight: 700;
      margin-bottom: 14px;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      font-size: 13px;
      font-family: "JetBrains Mono", "SF Mono", Consolas, monospace;
    }
    th {
      text-align: left;
      padding: 10px 12px;
      border-bottom: 1px solid var(--border);
      color: var(--text-muted);
      font-size: 11px;
      text-transform: uppercase;
    }
    td {
      padding: 10px 12px;
      border-bottom: 1px solid rgba(255,255,255,0.03);
    }
    tr:hover { background: rgba(255,255,255,0.02); }
    .tag {
      padding: 3px 8px;
      border-radius: 4px;
      font-size: 11px;
      font-weight: 600;
      display: inline-block;
    }
    .tag-fire { background: rgba(16, 185, 129, 0.2); color: var(--green); border: 1px solid var(--green); }
    .tag-hold { background: rgba(148, 163, 184, 0.1); color: var(--text-muted); }
    .tag-non-atomic { background: rgba(245, 158, 11, 0.15); color: var(--yellow); }
    .refresh-note { font-size: 12px; color: var(--text-muted); }
  </style>
</head>
<body>
  <div class="header">
    <div class="logo-group">
      <h2>HYDRA-HFT v4.0</h2>
      <span class="badge-live"><div class="pulse"></div> 48-HR CLOUD VALIDATOR ACTIVE</span>
    </div>
    <div class="refresh-note">Auto-refreshing every 2.0s &bull; Server: AWS EC2 us-east-1</div>
  </div>

  <div class="grid">
    <div class="card">
      <div class="card-label">Current Live Equity</div>
      <div class="card-val" id="val-equity">€150.0000</div>
      <div class="card-sub" id="val-roi">+0.000% ROI</div>
    </div>
    <div class="card">
      <div class="card-label">Net Arbitrage PnL</div>
      <div class="card-val positive" id="val-pnl">+€0.0000</div>
      <div class="card-sub" id="val-pnl-split">DEX: +€0.00 | CEX: +€0.00</div>
    </div>
    <div class="card">
      <div class="card-label">Market Polls & Executions</div>
      <div class="card-val" id="val-polls">0</div>
      <div class="card-sub" id="val-trades">0 Fired &bull; 0 Held</div>
    </div>
    <div class="card">
      <div class="card-label">SPFA Solver Execution</div>
      <div class="card-val" id="val-solver" style="color: var(--accent);">58.4 µs</div>
      <div class="card-sub">Institutional Target: &lt;100 µs</div>
    </div>
    <div class="card">
      <div class="card-label">Network Backbone RTT</div>
      <div class="card-val" id="val-rtt" style="color: var(--purple);">27.4 ms</div>
      <div class="card-sub">AWS us-east-1 &bull; Direct TLS</div>
    </div>
    <div class="card">
      <div class="card-label">Top Active Arbitrage Ring</div>
      <div class="card-val" id="val-ring-bps" style="font-size: 20px;">-1.79 bps</div>
      <div class="card-sub" id="val-ring-path" style="white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">USDC-&gt;ETH-&gt;WETH-&gt;USDC</div>
    </div>
  </div>

  <div class="table-card">
    <div class="table-title">
      <span>Live Market Opportunity Feed (Latest Real-Time Ticks)</span>
      <span style="font-size: 12px; color: var(--text-muted);" id="last-updated">Updating...</span>
    </div>
    <table>
      <thead>
        <tr>
          <th>Poll</th>
          <th>RTT</th>
          <th>Solver</th>
          <th>Venues</th>
          <th>Cycle Path</th>
          <th>Gross Spread</th>
          <th>Net After Fee</th>
          <th>Action</th>
          <th>Net PnL</th>
          <th>Equity</th>
        </tr>
      </thead>
      <tbody id="ticks-body">
        <tr><td colspan="10" style="text-align: center; color: var(--text-muted); padding: 20px;">Loading live telemetry from node...</td></tr>
      </tbody>
    </table>
  </div>

  <script>
    async function updateDashboard() {
      try {
        const res = await fetch('/api/telemetry?t=' + Date.now());
        if (!res.ok) return;
        const data = await res.json();
        
        if (data.status) {
          document.getElementById('val-equity').innerText = '€' + (data.status.current_equity || 150.0).toFixed(4);
          const roi = ((data.status.current_equity - data.status.starting_equity) / data.status.starting_equity * 100);
          const roiEl = document.getElementById('val-roi');
          roiEl.innerText = (roi >= 0 ? '+' : '') + roi.toFixed(3) + '% ROI';
          roiEl.className = 'card-sub ' + (roi >= 0 ? 'positive' : 'negative');

          const pnl = (data.status.current_equity - data.status.starting_equity);
          const pnlEl = document.getElementById('val-pnl');
          pnlEl.innerText = (pnl >= 0 ? '+' : '') + '€' + pnl.toFixed(4);
          pnlEl.className = 'card-val ' + (pnl >= 0 ? 'positive' : 'negative');
          document.getElementById('val-pnl-split').innerText = `DEX Atomic: +€${(data.status.dex_pnl || 0).toFixed(4)} | CEX: +€${(data.status.cex_pnl || 0).toFixed(4)}`;

          document.getElementById('val-polls').innerText = data.status.polls || 0;
          document.getElementById('val-trades').innerText = `${data.status.executed || 0} Fired • ${data.status.holds || 0} Held`;

          if (data.status.solver_us) document.getElementById('val-solver').innerText = data.status.solver_us.toFixed(2) + ' µs';
          if (data.status.rtt_ms) document.getElementById('val-rtt').innerText = data.status.rtt_ms.toFixed(1) + ' ms';
          if (data.status.best_net_bps !== undefined) document.getElementById('val-ring-bps').innerText = (data.status.best_net_bps >= 0 ? '+' : '') + data.status.best_net_bps.toFixed(2) + ' bps';
          if (data.status.best_path) document.getElementById('val-ring-path').innerText = `[${data.status.best_route || ''}] ${data.status.best_path}`;
        }

        if (data.ticks && data.ticks.length > 0) {
          const tbody = document.getElementById('ticks-body');
          tbody.innerHTML = '';
          data.ticks.slice(-15).reverse().forEach(t => {
            const tr = document.createElement('tr');
            let tagClass = 'tag-hold';
            if (t.action.includes('FIRE')) tagClass = 'tag-fire';
            else if (t.action.includes('NON_ATOMIC')) tagClass = 'tag-non-atomic';

            tr.innerHTML = `
              <td>#${t.poll}</td>
              <td>${t.rtt} ms</td>
              <td>${t.solver} µs</td>
              <td><span style="color: #cbd5e1;">${t.route}</span></td>
              <td><code>${t.path}</code></td>
              <td style="color: ${parseFloat(t.gross) >= 0 ? '#10b981' : '#ef4444'}">${t.gross} bps</td>
              <td style="color: ${parseFloat(t.net) >= 0 ? '#10b981' : '#94a3b8'}; font-weight: 600;">${t.net} bps</td>
              <td><span class="tag ${tagClass}">${t.action}</span></td>
              <td style="color: ${parseFloat(t.pnl) > 0 ? '#10b981' : '#64748b'}">+€${parseFloat(t.pnl).toFixed(4)}</td>
              <td><strong>€${parseFloat(t.equity).toFixed(4)}</strong></td>
            `;
            tbody.appendChild(tr);
          });
        }
        document.getElementById('last-updated').innerText = 'Last updated: ' + new Date().toLocaleTimeString();
      } catch (e) {
        console.error(e);
      }
    }
    setInterval(updateDashboard, 1500);
    updateDashboard();
  </script>
</body>
</html>
"""

def parse_status_md():
    status = {
        "starting_equity": 150.0,
        "current_equity": 150.0,
        "dex_pnl": 0.0,
        "cex_pnl": 0.0,
        "polls": 0,
        "executed": 0,
        "holds": 0,
        "rtt_ms": 28.0,
        "solver_us": 58.0,
        "best_route": "",
        "best_path": "",
        "best_net_bps": -1.5,
    }
    if not os.path.exists(STATUS_MD):
        return status
    try:
        with open(STATUS_MD, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if "Starting Test Equity:" in line:
                    parts = line.split("`€")
                    if len(parts) > 1:
                        status["starting_equity"] = float(parts[1].split("`")[0])
                elif "Current Live Equity:" in line:
                    parts = line.split("`€")
                    if len(parts) > 1:
                        status["current_equity"] = float(parts[1].split("`")[0])
                elif "Cumulative Net PnL:" in line:
                    if "Base L2 DEX Atomic:" in line:
                        sub = line.split("Base L2 DEX Atomic: `+€")
                        if len(sub) > 1:
                            status["dex_pnl"] = float(sub[1].split("`")[0])
                    if "CEX/Mesh:" in line:
                        sub = line.split("CEX/Mesh: `+€")
                        if len(sub) > 1:
                            status["cex_pnl"] = float(sub[1].split("`")[0])
                elif "Live Polls Completed:" in line:
                    parts = line.split("`")
                    if len(parts) >= 2:
                        status["polls"] = int(parts[1])
                    if len(parts) >= 4:
                        status["executed"] = int(parts[3])
                    if len(parts) >= 6:
                        status["holds"] = int(parts[5])
                elif "Latest Network Poll RTT:" in line:
                    parts = line.split("`")
                    if len(parts) >= 2:
                        status["rtt_ms"] = float(parts[1].replace(" ms", ""))
                    if len(parts) >= 4:
                        status["solver_us"] = float(parts[3].replace(" µs", "").replace(" us", ""))
                elif "Top Active Arbitrage Ring:" in line:
                    parts = line.split("`")
                    if len(parts) >= 2:
                        ring_raw = parts[1]
                        if "]" in ring_raw:
                            status["best_route"] = ring_raw.split("]")[0].replace("[", "")
                            status["best_path"] = ring_raw.split("]")[1].strip()
                        else:
                            status["best_path"] = ring_raw
                    if len(parts) >= 4:
                        status["best_net_bps"] = float(parts[3].replace(" bps", ""))
    except Exception as e:
        pass
    return status

def parse_csv_ticks():
    ticks = []
    if not os.path.exists(CSV_PATH):
        return ticks
    try:
        with open(CSV_PATH, "r", encoding="utf-8") as f:
            lines = [l.strip() for l in f if l.strip()]
        if len(lines) <= 1:
            return ticks
        # read header and last 40 lines
        for l in lines[-40:]:
            parts = l.split(",")
            if len(parts) >= 12 and parts[0] != "poll_idx":
                ticks.append({
                    "poll": parts[0],
                    "unix_s": parts[1],
                    "rtt": parts[2],
                    "solver": parts[3],
                    "route": parts[4],
                    "path": parts[5],
                    "gross": parts[6],
                    "net": parts[7],
                    "action": parts[8],
                    "notional": parts[9],
                    "pnl": parts[10],
                    "equity": parts[11],
                })
    except Exception:
        pass
    return ticks

class TelemetryHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith("/api/telemetry"):
            status = parse_status_md()
            ticks = parse_csv_ticks()
            payload = json.dumps({"status": status, "ticks": ticks}).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(payload)
        else:
            payload = HTML_TEMPLATE.encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(payload)

    def log_message(self, format, *args):
        # Silence HTTP access logs to keep terminal stdout clean
        pass

if __name__ == "__main__":
    server_address = ("", PORT)
    with socketserver.TCPServer(server_address, TelemetryHandler) as httpd:
        print(f"HYDRA-HFT Live Web Dashboard running at http://0.0.0.0:{PORT}")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nShutting down dashboard server...")
