#!/usr/bin/env python3
"""
HYDRA-HFT v4.0 — Live Status and Metrics Reporter
"""
import os
import json
import urllib.request

WALLET = "0x5e2b67e63d5Fe5C42139c5A10A8b57BF24e4f1A8"
EXECUTOR = "0x2d1a594e982ee19d2718992991d3835bfec5d4ac"
RPC_URL = "https://mainnet.base.org"

def rpc_call(method, params):
    try:
        data = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
        req = urllib.request.Request(RPC_URL, data=data, headers={"Content-Type": "application/json", "User-Agent": "Mozilla/5.0"})
        with urllib.request.urlopen(req, timeout=5) as resp:
            res = json.loads(resp.read().decode())
            return res.get("result")
    except Exception as e:
        return None

def main():
    bal_hex = rpc_call("eth_getBalance", [WALLET, "latest"])
    bal_eth = int(bal_hex, 16) / 1e18 if bal_hex else 0.0

    nonce_hex = rpc_call("eth_getTransactionCount", [WALLET, "pending"])
    nonce = int(nonce_hex, 16) if nonce_hex else 0

    print(f"WALLET_BALANCE_ETH={bal_eth:.6f}")
    print(f"WALLET_NONCE={nonce}")
    
    # Check live trades log
    live_trades_file = os.path.join("logs", "live_onchain_trades.jsonl")
    trade_count = 0
    if os.path.exists(live_trades_file):
        with open(live_trades_file, "r") as f:
            trade_count = sum(1 for line in f if line.strip())
    print(f"LIVE_TRADES_COUNT={trade_count}")

if __name__ == "__main__":
    main()
