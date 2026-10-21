import os
import sys
import json
import time
import urllib.request
from decimal import Decimal
from dotenv import load_dotenv
from eth_account import Account
from eth_abi import encode

BASE_RPC_LIST = [
    "https://base-rpc.publicnode.com",
    "https://mainnet.base.org",
    "https://base.llamarpc.com"
]

def rpc_call(method, params, rpc_idx=0):
    payload = json.dumps({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    }).encode("utf-8")
    
    for i in range(len(BASE_RPC_LIST)):
        url = BASE_RPC_LIST[(rpc_idx + i) % len(BASE_RPC_LIST)]
        req = urllib.request.Request(
            url,
            data=payload,
            headers={"Content-Type": "application/json", "User-Agent": "Hydra-Deployer/1.0"}
        )
        try:
            with urllib.request.urlopen(req, timeout=8) as res:
                data = json.loads(res.read().decode("utf-8"))
                if "error" in data:
                    continue
                return data.get("result")
        except Exception:
            continue
    raise RuntimeError(f"All Base RPCs failed for {method}")

def main():
    load_dotenv()
    pk = os.getenv("HYDRA_HOT_WALLET_PRIVKEY")
    if not pk:
        print("[!] Error: HYDRA_HOT_WALLET_PRIVKEY not found in .env")
        sys.exit(1)
        
    acct = Account.from_key(pk)
    wallet_addr = acct.address
    print(f"[*] Deployer Hot Wallet Address: {wallet_addr}")
    
    # 1. Check ETH balance on Base
    bal_hex = rpc_call("eth_getBalance", [wallet_addr, "latest"])
    bal_wei = int(bal_hex, 16)
    bal_eth = bal_wei / 1e18
    print(f"[*] Current Base ETH Balance: {bal_eth:.6f} ETH ({bal_wei} wei)")
    
    min_required_wei = int(0.0008 * 1e18) # ~0.0008 ETH (~$2.00) for contract deploy
    if bal_wei < min_required_wei:
        print(f"[!] Insufficient ETH on Base! Required at least ~0.001 ETH to deploy.")
        print(f"    Please ensure your $16.99 Cash in Phantom is converted to ETH on the BASE network.")
        return False

    # 2. Load compiled artifact
    artifact_path = "contracts/out/HydraAtomicExecutor.sol/HydraAtomicExecutor.json"
    if not os.path.exists(artifact_path):
        print(f"[!] Error: Artifact {artifact_path} not found!")
        sys.exit(1)
        
    with open(artifact_path, "r") as f:
        artifact = json.load(f)
        
    bytecode = artifact["bytecode"]["object"]
    if bytecode.startswith("0x"):
        bytecode = bytecode[2:]
        
    aave_v3_pool = "0xA238Dd80C259a72e81d7e4664a9801593F98d1c5"
    balancer_vault = "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
    
    args_encoded = encode(["address", "address", "address"], [wallet_addr, aave_v3_pool, balancer_vault]).hex()
    deploy_data = "0x" + bytecode + args_encoded
    
    print(f"[*] Prepared deployment payload ({len(deploy_data) // 2} bytes)...")
    
    # 3. Get Nonce
    nonce_hex = rpc_call("eth_getTransactionCount", [wallet_addr, "pending"])
    nonce = int(nonce_hex, 16)
    print(f"[*] Nonce: {nonce}")
    
    # 4. Get Gas Pricing (Base L2)
    gas_price_hex = rpc_call("eth_gasPrice", [])
    gas_price = int(gas_price_hex, 16)
    # Ensure a slight buffer for fast execution
    gas_price = max(gas_price, int(0.05 * 1e9)) # min 0.05 gwei
    
    # Estimate Gas
    try:
        est_gas_hex = rpc_call("eth_estimateGas", [{
            "from": wallet_addr,
            "data": deploy_data
        }])
        gas_limit = int(int(est_gas_hex, 16) * 1.25) # 25% safety buffer
        print(f"[*] Estimated Gas: {int(est_gas_hex, 16)} -> Limit: {gas_limit}")
    except Exception as e:
        gas_limit = 3500000
        print(f"[~] Gas estimation warning: {e}. Using fallback limit: {gas_limit}")
        
    est_cost_eth = (gas_limit * gas_price) / 1e18
    print(f"[*] Max Gas Cost for Deploy: {est_cost_eth:.6f} ETH (~${est_cost_eth * 2600:.3f})")
    
    # 5. Build and Sign Transaction
    tx = {
        "nonce": nonce,
        "gasPrice": gas_price,
        "gas": gas_limit,
        "to": "",  # Empty for contract creation
        "value": 0,
        "data": deploy_data,
        "chainId": 8453 # Base Mainnet
    }
    
    print("[*] Signing contract deployment transaction...")
    signed_tx = acct.sign_transaction(tx)
    raw_tx_hex = "0x" + signed_tx.raw_transaction.hex()
    
    # 6. Broadcast
    print("[*] Broadcasting deployment to Base Mainnet sequencer...")
    tx_hash = rpc_call("eth_sendRawTransaction", [raw_tx_hex])
    print(f"[+] Transaction broadcasted! TxHash: {tx_hash}")
    print(f"    View on BaseScan: https://basescan.org/tx/{tx_hash}")
    
    # 7. Wait for receipt
    print("[*] Waiting for on-chain confirmation (Base block time ~2s)...")
    deployed_address = None
    for _ in range(30):
        time.sleep(2)
        receipt = rpc_call("eth_getTransactionReceipt", [tx_hash])
        if receipt:
            status = int(receipt.get("status", "0x0"), 16)
            if status == 1:
                deployed_address = receipt.get("contractAddress")
                gas_used = int(receipt.get("gasUsed", "0x0"), 16)
                actual_fee = (gas_used * gas_price) / 1e18
                print(f"\n=======================================================")
                print(f"[SUCCESS] HydraAtomicExecutor successfully deployed!")
                print(f"Contract Address: {deployed_address}")
                print(f"Gas Used: {gas_used} ({actual_fee:.6f} ETH / ~${actual_fee * 2600:.4f})")
                print(f"BaseScan: https://basescan.org/address/{deployed_address}")
                print(f"=======================================================\n")
                break
            else:
                print(f"[!] Deployment transaction reverted! Status: {status}")
                return False
                
    if not deployed_address:
        print("[!] Timeout waiting for receipt. Check tx on BaseScan.")
        return False
        
    # 8. Save to .env
    with open(".env", "r") as f:
        env_lines = f.readlines()
        
    updated = False
    new_lines = []
    for line in env_lines:
        if line.startswith("HYDRA_EXECUTOR_CONTRACT="):
            new_lines.append(f"HYDRA_EXECUTOR_CONTRACT={deployed_address}\n")
            updated = True
        else:
            new_lines.append(line)
            
    if not updated:
        new_lines.append(f"HYDRA_EXECUTOR_CONTRACT={deployed_address}\n")
        
    with open(".env", "w") as f:
        f.writelines(new_lines)
        
    print(f"[+] Saved HYDRA_EXECUTOR_CONTRACT={deployed_address} to .env")
    return True

if __name__ == "__main__":
    main()
