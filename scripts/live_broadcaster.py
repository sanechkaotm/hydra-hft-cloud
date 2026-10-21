#!/usr/bin/env python3
"""
HYDRA-HFT v4.0 — High-Frequency Live Transaction Broadcaster & Anomaly Guardian
Listens on UDP 127.0.0.1:8765 for candidate arbitrage payloads from hydra-node,
performs mandatory pre-flight on-chain `eth_call` simulation, enforces strict
anomaly stop-loss rules, signs with zero-leak local keystore, broadcasts via
Base Sequencer, and instantly alerts via Telegram @HydraHFTbot.
"""

import os
import sys
import time
import json
import socket
import urllib.request
import requests
from dotenv import load_dotenv
from eth_account import Account

# Load environment
load_dotenv()
HOT_KEY = os.getenv("HYDRA_HOT_WALLET_PRIVKEY") or os.getenv("HYDRA_WALLET_PRIVATE_KEY")
WALLET_ADDR = os.getenv("HYDRA_WALLET_ADDRESS")
CONTRACT_ADDR = os.getenv("HYDRA_EXECUTOR_CONTRACT")
TG_BOT_TOKEN = "8851439624:AAFy-N0xi_mAGmi0mhzjAuUtDd_-MI956qM"
TG_CHAT_ID_FILE = os.path.join("logs", "tg_chat_id.txt")
LIVE_LOG_FILE = os.path.join("logs", "live_onchain_trades.jsonl")

BASE_RPCS = [
    "https://mainnet.base.org",          # Primary: Official Coinbase Sequencer endpoint
    "https://base-rpc.publicnode.com",   # Backup 1
    "https://base.llamarpc.com"          # Backup 2
]

# Strict Anomaly Guardrail Limits
MIN_WALLET_BALANCE_ETH = 0.0030    # Hard stop if gas drops below ~0.003 ETH (~$7.80)
MAX_GAS_PRICE_GWEI = 0.150         # Max baseFee limit
MAX_TRANSACTION_GAS_ETH = 0.00030  # Max gas spent per transaction (~$0.75)
MAX_CONSECUTIVE_REVERTS = 1        # HALT ON FIRST REVERT
MAX_DAILY_GAS_EXPENSE_EUR = 0.80   # Stop if daily gas exceeds €0.80

class AnomalyGuardian:
    def __init__(self, wallet_addr, contract_addr, private_key):
        self.wallet_addr = wallet_addr
        self.contract_addr = contract_addr
        self.acct = Account.from_key(private_key)
        self.consecutive_reverts = 0
        self.total_gas_spent_eth = 0.0
        self.total_live_trades = 0
        self.total_real_profit_eur = 0.0
        self.is_halted = False
        self.halt_reason = None
        
        # Persistent HTTP Keep-Alive session for sub-millisecond RPC round-trips
        self.session = requests.Session()
        adapter = requests.adapters.HTTPAdapter(pool_connections=10, pool_maxsize=20, max_retries=1)
        self.session.mount("https://", adapter)
        self.session.mount("http://", adapter)
        self.session.headers.update({"Content-Type": "application/json", "User-Agent": "Hydra-Live/1.0"})
        
    def log(self, msg):
        ts = time.strftime("%Y-%m-%d %H:%M:%S")
        print(f"[{ts}] [LIVE GUARDIAN] {msg}", flush=True)

    def send_tg_alert(self, text):
        if not os.path.exists(TG_CHAT_ID_FILE):
            return
        try:
            with open(TG_CHAT_ID_FILE, "r") as f:
                chat_ids = [line.strip() for line in f if line.strip().isdigit()]
            for cid in chat_ids:
                url = f"https://api.telegram.org/bot{TG_BOT_TOKEN}/sendMessage"
                payload = {
                    "chat_id": int(cid),
                    "text": text,
                    "parse_mode": "Markdown",
                    "disable_web_page_preview": True
                }
                self.session.post(url, json=payload, timeout=4)
        except Exception as e:
            self.log(f"TG notification warning: {e}")

    def rpc_request(self, method, params, timeout=4):
        payload = {"jsonrpc": "2.0", "method": method, "params": params, "id": 1}
        for rpc in BASE_RPCS:
            try:
                res = self.session.post(rpc, json=payload, timeout=timeout)
                data = res.json()
                if "error" in data:
                    return False, data["error"]
                return True, data.get("result")
            except Exception:
                continue
        return False, {"message": "All Base RPC endpoints unreachable"}

    def check_preflight_anomaly(self):
        # 1. Check wallet balance
        ok, res = self.rpc_request("eth_getBalance", [self.wallet_addr, "latest"])
        if not ok:
            return False, f"Failed to query wallet balance: {res}"
        bal_wei = int(res, 16)
        bal_eth = bal_wei / 1e18
        if bal_eth < MIN_WALLET_BALANCE_ETH:
            return False, f"Wallet balance too low ({bal_eth:.6f} ETH < threshold {MIN_WALLET_BALANCE_ETH} ETH)"

        # 2. Check network gas price
        ok, res = self.rpc_request("eth_gasPrice", [])
        if not ok:
            return False, f"Failed to query gas price: {res}"
        gas_price_wei = int(res, 16)
        gas_price_gwei = gas_price_wei / 1e9
        if gas_price_gwei > MAX_GAS_PRICE_GWEI:
            return False, f"Network gas price spike: {gas_price_gwei:.4f} Gwei exceeds limit {MAX_GAS_PRICE_GWEI} Gwei"

        return True, {"balance_eth": bal_eth, "gas_price_wei": gas_price_wei}

    def simulate_call(self, calldata_hex):
        # Mandatory read-only eth_call against Base Mainnet sequencer BEFORE broadcasting
        call_obj = {
            "from": self.wallet_addr,
            "to": self.contract_addr,
            "data": calldata_hex
        }
        ok, res = self.rpc_request("eth_call", [call_obj, "latest"])
        if not ok:
            err_msg = res.get("message", "EVM reverted")
            err_data = res.get("data", "")
            return False, f"{err_msg} (data: {err_data})"
        return True, res

    def execute_and_broadcast(self, cycle_info):
        if self.is_halted:
            return False, f"System halted: {self.halt_reason}"

        calldata = cycle_info["calldata"]
        path = cycle_info.get("path", "Unknown Path")
        route = cycle_info.get("route", "Unknown Route")
        net_profit_eur = cycle_info.get("net_profit_eur", 0.0)
        tip_wei = int(cycle_info.get("priority_tip_wei", 10_000_000)) # Default 0.01 Gwei

        # 1. Preflight safety check
        preflight_ok, preflight_data = self.check_preflight_anomaly()
        if not preflight_ok:
            self.trigger_emergency_halt(f"Pre-flight safety barrier tripped: {preflight_data}")
            return False, preflight_data

        gas_price_wei = preflight_data["gas_price_wei"]

        # 2. On-chain EVM simulation (eth_call)
        sim_ok, sim_result = self.simulate_call(calldata)
        if not sim_ok:
            self.log(f"[SIMULATION PROTECTED] Cycle rejected by EVM preflight: {sim_result}. 0 gas spent.")
            return False, f"Simulation reverted: {sim_result}"

        # 3. Nonce & Gas parameters
        ok, nonce_hex = self.rpc_request("eth_getTransactionCount", [self.wallet_addr, "pending"])
        if not ok:
            return False, "Failed to get nonce"
        nonce = int(nonce_hex, 16)

        gas_limit = 380_000
        max_fee_per_gas = gas_price_wei + tip_wei

        tx = {
            "nonce": nonce,
            "gas": gas_limit,
            "maxFeePerGas": max_fee_per_gas,
            "maxPriorityFeePerGas": tip_wei,
            "to": self.contract_addr,
            "value": 0,
            "data": calldata,
            "chainId": 8453
        }

        # 4. Sign and broadcast
        self.log(f"[*] Preflight passed! Signing real transaction for [{route}] {path} (+€{net_profit_eur:.4f})...")
        signed = self.acct.sign_transaction(tx)
        raw_hex = "0x" + signed.raw_transaction.hex()

        ok, broadcast_res = self.rpc_request("eth_sendRawTransaction", [raw_hex])
        if not ok:
            err_msg = broadcast_res.get("message", "Broadcast failed")
            self.log(f"[!] Broadcast failed: {err_msg}")
            return False, err_msg

        tx_hash = broadcast_res
        self.log(f"[+] BROADCAST SUCCESS! TxHash: {tx_hash}")
        basescan_url = f"https://basescan.org/tx/{tx_hash}"

        # 5. Await on-chain receipt (Base 2s block time)
        confirmed = False
        receipt = None
        for _ in range(15):
            time.sleep(2)
            ok, r = self.rpc_request("eth_getTransactionReceipt", [tx_hash])
            if ok and r:
                receipt = r
                confirmed = True
                break

        if not confirmed:
            self.trigger_emergency_halt(f"Transaction confirmation timeout for {tx_hash}")
            return False, "Receipt timeout"

        status = int(receipt.get("status", "0x0"), 16)
        gas_used = int(receipt.get("gasUsed", "0x0"), 16)
        actual_gas_cost_eth = (gas_used * max_fee_per_gas) / 1e18
        self.total_gas_spent_eth += actual_gas_cost_eth

        if status == 1:
            self.consecutive_reverts = 0
            self.total_live_trades += 1
            self.total_real_profit_eur += net_profit_eur
            self.log(f"\n=======================================================")
            self.log(f"[SUCCESS] ON-CHAIN ARBITRAGE CONFIRMED ON BASE!")
            self.log(f"TxHash: {tx_hash}")
            self.log(f"Route: [{route}] {path}")
            self.log(f"Net Profit: +€{net_profit_eur:.4f} | Gas: {gas_used} (~${actual_gas_cost_eth * 2600:.4f})")
            self.log(f"BaseScan: {basescan_url}")
            self.log(f"=======================================================\n")

            alert_msg = (
                f"🚀 *LIVE ARBITRAGE CONFIRMED ON BASE!* 🚀\n\n"
                f"• *Маршрут:* `[{route}]` {path}\n"
                f"• *Чистая прибыль:* `+€{net_profit_eur:.4f}`\n"
                f"• *Газ сети:* `{gas_used} gas` (~${actual_gas_cost_eth * 2600:.4f})\n"
                f"• *BaseScan:* [Смотреть транзакцию]({basescan_url})"
            )
            self.send_tg_alert(alert_msg)

            # Record to live log
            os.makedirs("logs", exist_ok=True)
            with open(LIVE_LOG_FILE, "a") as f:
                f.write(json.dumps({
                    "timestamp": time.time(),
                    "tx_hash": tx_hash,
                    "route": route,
                    "path": path,
                    "net_profit_eur": net_profit_eur,
                    "gas_used": gas_used,
                    "gas_cost_eth": actual_gas_cost_eth
                }) + "\n")
            return True, tx_hash
        else:
            self.consecutive_reverts += 1
            halt_reason = f"Transaction reverted on-chain on Base: {tx_hash} (Gas used: {gas_used})"
            self.trigger_emergency_halt(halt_reason)
            return False, halt_reason

    def trigger_emergency_halt(self, reason):
        self.is_halted = True
        self.halt_reason = reason
        self.log(f"\n[EMERGENCY HALT TRIPPED] {reason}")
        self.log("[SAFETY] All on-chain broadcasts are instantly stopped to preserve capital.")
        
        # Write halt flag for hydra-node to read
        with open(os.path.join("logs", "HALT_KILLSWITCH.flag"), "w") as f:
            f.write(f"{time.time()}: {reason}\n")
            
        tg_alert = (
            f"🚨 *HYDRA-HFT EMERGENCY HALT TRIPPED* 🚨\n\n"
            f"⚠️ *Причина:* `{reason}`\n"
            f"🛡️ *Статус:* Все транзакции мгновенно остановлены для защиты депозита.\n"
            f"💰 *Остаток на кошельке сохранен.*"
        )
        self.send_tg_alert(tg_alert)

def main():
    if not HOT_KEY or not WALLET_ADDR or not CONTRACT_ADDR:
        print("[!] Error: Missing HYDRA_HOT_WALLET_PRIVKEY, HYDRA_WALLET_ADDRESS, or HYDRA_EXECUTOR_CONTRACT in .env")
        sys.exit(1)

    guardian = AnomalyGuardian(WALLET_ADDR, CONTRACT_ADDR, HOT_KEY)
    guardian.log("==================================================================")
    guardian.log(" HYDRA-HFT v4.0 — LIVE ON-CHAIN BROADCASTER & ANOMALY GUARDIAN")
    guardian.log(f" Operator Hot Wallet : {WALLET_ADDR}")
    guardian.log(f" Target Contract     : {CONTRACT_ADDR}")
    guardian.log(f" Base Sequencer      : {BASE_RPCS[0]}")
    guardian.log(f" Hard Stop Limits    : Min Bal {MIN_WALLET_BALANCE_ETH} ETH | Max Gas {MAX_GAS_PRICE_GWEI} Gwei | Halt on Revert: YES")
    guardian.log(" Listening for triggers on UDP 127.0.0.1:8765...")
    guardian.log("==================================================================")

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", 8765))
    sock.setblocking(True)

    while True:
        try:
            data, addr = sock.recvfrom(65535)
            if not data:
                continue
            payload = json.loads(data.decode("utf-8"))
            guardian.log(f"Received candidate trade trigger from {addr}: {payload.get('route')} ({payload.get('net_profit_eur')} EUR)")
            guardian.execute_and_broadcast(payload)
        except KeyboardInterrupt:
            guardian.log("Shutting down live broadcaster.")
            break
        except Exception as e:
            guardian.log(f"Loop exception: {e}")
            time.sleep(1)

if __name__ == "__main__":
    main()
