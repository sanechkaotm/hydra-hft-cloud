#!/usr/bin/env python3
"""
Hydra-HFT v4.0 — Secure EVM Hot Wallet Manager for Base Mainnet (Chain ID: 8453)
Generates, stores, and monitors the dedicated Hot Wallet address on Base.
"""

import os
import sys
import json
import secrets
import urllib.request
import coincurve
from Crypto.Hash import keccak

try:
    if sys.stdout:
        sys.stdout.reconfigure(encoding="utf-8")
    if sys.stderr:
        sys.stderr.reconfigure(encoding="utf-8")
except Exception:
    pass

ENV_PATH = ".env"
BASE_RPC_URL = "https://base-rpc.publicnode.com"


def to_checksum_address(addr: str) -> str:
    addr = addr.lower().replace("0x", "")
    k = keccak.new(digest_bits=256)
    k.update(addr.encode("ascii"))
    h = k.digest().hex()
    return "0x" + "".join(c.upper() if int(h[i], 16) >= 8 else c.lower() for i, c in enumerate(addr))


def get_or_create_wallet() -> tuple:
    priv_hex = None
    addr = None

    if os.path.exists(ENV_PATH):
        with open(ENV_PATH, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line.startswith("HYDRA_HOT_WALLET_PRIVKEY="):
                    priv_hex = line.split("=", 1)[1].strip()
                elif line.startswith("HYDRA_EXECUTOR_ADDRESS="):
                    addr = line.split("=", 1)[1].strip()

    if not priv_hex:
        # Generate cryptographically secure 256-bit private key
        priv_bytes = secrets.token_bytes(32)
        priv_hex = "0x" + priv_bytes.hex()
        pub_bytes = coincurve.PublicKey.from_valid_secret(priv_bytes).format(compressed=False)[1:]
        k = keccak.new(digest_bits=256)
        k.update(pub_bytes)
        raw_addr = "0x" + k.digest()[-20:].hex()
        addr = to_checksum_address(raw_addr)

        # Write safely to .env
        env_content = (
            f"# HYDRA-HFT v4.0 — Production Base Mainnet Wallet Configuration\n"
            f"# Generated locally. Never share or commit this file.\n"
            f"HYDRA_HOT_WALLET_PRIVKEY={priv_hex}\n"
            f"HYDRA_WALLET_PRIVATE_KEY={priv_hex}\n"
            f"HYDRA_EXECUTOR_ADDRESS={addr}\n"
            f"HYDRA_INITIAL_CAPITAL_EUR=15.00\n"
            f"HYDRA_NODE_ROLE=frankfurt\n"
            f"HYDRA_BASE_RPC_URL=https://base-rpc.publicnode.com\n"
        )
        with open(ENV_PATH, "w", encoding="utf-8") as f:
            f.write(env_content)

    return priv_hex, addr


def check_onchain_balance(addr: str) -> tuple:
    payload = json.dumps({
        "jsonrpc": "2.0",
        "method": "eth_getBalance",
        "params": [addr, "latest"],
        "id": 1
    }).encode("utf-8")

    req = urllib.request.Request(
        BASE_RPC_URL,
        data=payload,
        headers={"Content-Type": "application/json", "User-Agent": "Hydra/4.0"}
    )
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            hex_bal = data.get("result", "0x0")
            wei = int(hex_bal, 16)
            eth = wei / 1e18
            return wei, eth
    except Exception as e:
        return 0, 0.0


def main():
    priv, addr = get_or_create_wallet()
    wei, eth = check_onchain_balance(addr)
    eth_price_approx = 2450.0
    eur_val = eth * eth_price_approx

    print("==========================================================================================")
    print(" HYDRA-HFT v4.0 — SECURE HOT WALLET GENERATOR (BASE MAINNET)")
    print("==========================================================================================")
    print(f" • Сеть блокчейна  : Base Mainnet (Layer 2, Chain ID: 8453)")
    print(f" • ПУБЛИЧНЫЙ АДРЕС : {addr}")
    print(f" • Баланс на Base  : {eth:.6f} ETH (~€{eur_val:.2f})")
    print(f" • Файл конфигурации: {os.path.abspath(ENV_PATH)}")
    print("------------------------------------------------------------------------------------------")
    if eth == 0.0:
        needed_eth = 15.0 / eth_price_approx
        print(f" [СТАТУС]: ОЖИДАЕТСЯ ПОПОЛНЕНИЕ ДЕПОЗИТА")
        print(f" Отправьте 15 евро в ETH сети Base (примерно {needed_eth:.5f} ETH) на этот адрес:")
        print(f" >>> {addr} <<<")
    else:
        print(f" [СТАТУС]: ДЕПОЗИТ ОБНАРУЖЕН! ({eth:.6f} ETH)")
    print("==========================================================================================")


if __name__ == "__main__":
    main()
