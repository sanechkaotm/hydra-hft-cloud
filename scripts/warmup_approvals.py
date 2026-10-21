#!/usr/bin/env python3
"""
HYDRA-HFT v4.1 — Gas Warmup & Pre-Approval Optimiser
Pre-approves all active DEX routers & lending vaults for infinite allowance (type(uint256).max)
via HydraAtomicExecutor.preApproveTokens().
Executing this once eliminates SSTORE approval gas on the hot path, saving ~20,000–60,000 gas per trade!
"""

import os
import sys
import time
import json
import requests
from dotenv import load_dotenv
from eth_account import Account
from eth_abi import encode

load_dotenv()
PRIV_KEY = os.getenv("HYDRA_HOT_WALLET_PRIVKEY") or os.getenv("HYDRA_WALLET_PRIVATE_KEY")
WALLET_ADDR = os.getenv("HYDRA_WALLET_ADDRESS")
CONTRACT_ADDR = os.getenv("HYDRA_EXECUTOR_CONTRACT")
RPC_URL = "https://mainnet.base.org"

if not PRIV_KEY or not CONTRACT_ADDR:
    print("[!] Missing HYDRA_HOT_WALLET_PRIVKEY or HYDRA_EXECUTOR_CONTRACT in .env")
    sys.exit(1)

acct = Account.from_key(PRIV_KEY)

# Key Base Mainnet Tokens
TOKENS = [
    "0x4200000000000000000000000000000000000006", # WETH
    "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", # USDC
    "0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42", # EURC
    "0x940181a94A35A4569E4529A3CDfB74e38FD98631", # AERO
    "0x0b3e328455c4059EEb9e3f84b5543F74E24e7E1b", # VIRTUAL
    "0xcbB7C0000aB88B473b1f5aFd9ef808440eed33Bf", # CBBTC
    "0x532f27101965dd16442e59d40670faf5ebb142e4", # BRETT
    "0x4ed4e862860bed51a9570b96d89af5e1b0efefed", # DEGEN
]

# Spenders
SPENDERS = [
    "0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43", # Aerodrome Router
    "0x2626664c2603336E57B271c5C0b26F421741e481", # UniV3 SwapRouter02
    "0xBA12222222228d8Ba445958a75a0704d566BF2C8", # Balancer Vault
    "0xA238Dd80C259a72e81d7e4664a9801593F98d1c5", # Aave V3 Pool
]

def rpc(method, params):
    res = requests.post(RPC_URL, json={"jsonrpc": "2.0", "method": method, "params": params, "id": 1}, timeout=5)
    data = res.json()
    if "error" in data:
        raise RuntimeError(data["error"])
    return data.get("result")

def main():
    print("==========================================================================")
    print(" HYDRA-HFT v4.1 — ONE-TIME ZERO-GAS PRE-APPROVAL WARMUP")
    print(f" Target Contract: {CONTRACT_ADDR}")
    print(f" Executor Wallet: {acct.address}")
    print("==========================================================================")

    # Build token-spender pairs
    token_list = []
    spender_list = []
    for t in TOKENS:
        for s in SPENDERS:
            token_list.append(t)
            spender_list.append(s)

    print(f"[*] Preparing batch approvals for {len(token_list)} pairs...")

    # Method selector: preApproveTokens(address[],address[]) -> 0x6e7e4088
    # keccak256("preApproveTokens(address[],address[])")[:4]
    from Crypto.Hash import keccak
    k = keccak.new(digest_bits=256)
    k.update(b"preApproveTokens(address[],address[])")
    selector = k.digest()[:4]

    encoded_args = encode(["address[]", "address[]"], [token_list, spender_list])
    calldata = "0x" + selector.hex() + encoded_args.hex()

    # Preflight simulation
    print("[*] Simulating execution on Base EVM via eth_call...")
    try:
        sim_res = rpc("eth_call", [{"from": acct.address, "to": CONTRACT_ADDR, "data": calldata}, "latest"])
        print(f"[+] Simulation passed! (Result: {sim_res})")
    except Exception as e:
        print(f"[!] Simulation failed: {e}")
        return

    # Gas & Nonce
    nonce_hex = rpc("eth_getTransactionCount", [acct.address, "pending"])
    nonce = int(nonce_hex, 16)
    gas_price_hex = rpc("eth_gasPrice", [])
    gas_price = int(int(gas_price_hex, 16) * 1.1)

    est_gas = rpc("eth_estimateGas", [{"from": acct.address, "to": CONTRACT_ADDR, "data": calldata}])
    gas_limit = int(int(est_gas, 16) * 1.2)
    cost_eth = (gas_limit * gas_price) / 1e18
    print(f"[*] Estimated Gas: {int(est_gas, 16)} -> Cost: {cost_eth:.6f} ETH (~${cost_eth * 2600:.4f})")

    from eth_utils import to_checksum_address
    tx = {
        "nonce": nonce,
        "gasPrice": gas_price,
        "gas": gas_limit,
        "to": to_checksum_address(CONTRACT_ADDR),
        "value": 0,
        "data": calldata,
        "chainId": 8453
    }

    signed_tx = acct.sign_transaction(tx)
    raw_hex = "0x" + signed_tx.raw_transaction.hex()

    print("[*] Broadcasting warmup transaction to Base Sequencer...")
    tx_hash = rpc("eth_sendRawTransaction", [raw_hex])
    print(f"[+] Broadcast successful! TxHash: {tx_hash}")
    print(f"    BaseScan: https://basescan.org/tx/{tx_hash}")

    print("[*] Waiting for on-chain confirmation...")
    for _ in range(20):
        time.sleep(2)
        receipt = rpc("eth_getTransactionReceipt", [tx_hash])
        if receipt:
            status = int(receipt.get("status", "0x0"), 16)
            if status == 1:
                print(f"[SUCCESS] All {len(token_list)} token approvals permanently cached in contract!")
                print(" -> Hot-path arbitrage swaps will now save ~25,000–60,000 gas per execution!")
                return
            else:
                print(f"[!] Transaction reverted on-chain. Status: {status}")
                return

if __name__ == "__main__":
    main()
