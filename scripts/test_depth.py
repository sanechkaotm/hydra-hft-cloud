from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))

quoter = Web3.to_checksum_address("0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a")
usdc = Web3.to_checksum_address("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")
usdt = Web3.to_checksum_address("0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2")
weth = Web3.to_checksum_address("0x4200000000000000000000000000000000000006")
cbeth = Web3.to_checksum_address("0x2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22")

quoter_abi = [
    {
        "inputs": [
            {
                "components": [
                    {"internalType": "address", "name": "tokenIn", "type": "address"},
                    {"internalType": "address", "name": "tokenOut", "type": "address"},
                    {"internalType": "uint256", "name": "amountIn", "type": "uint256"},
                    {"internalType": "uint24", "name": "fee", "type": "uint24"},
                    {"internalType": "uint160", "name": "sqrtPriceLimitX96", "type": "uint160"}
                ],
                "internalType": "struct IQuoterV2.QuoteExactInputSingleParams",
                "name": "params",
                "type": "tuple"
            }
        ],
        "name": "quoteExactInputSingle",
        "outputs": [
            {"internalType": "uint256", "name": "amountOut", "type": "uint256"},
            {"internalType": "uint160", "name": "sqrtPriceX96After", "type": "uint160"},
            {"internalType": "uint32", "name": "initializedTicksCrossed", "type": "uint32"},
            {"internalType": "uint256", "name": "gasEstimate", "type": "uint256"}
        ],
        "stateMutability": "nonpayable",
        "type": "function"
    }
]

q = w3.eth.contract(address=quoter, abi=quoter_abi)

print("--- Testing USDC -> USDT 1-bps (fee=100) ---")
for amt in [1000, 5000, 10000, 50000]:
    try:
        res = q.functions.quoteExactInputSingle({
            "tokenIn": usdc,
            "tokenOut": usdt,
            "amountIn": amt * 10**6,
            "fee": 100,
            "sqrtPriceLimitX96": 0
        }).call()
        out = res[0] / 1e6
        print(f"In ${amt:5} USDC -> Out ${out:8.2f} USDT (rate: {out/amt:.6f})")
    except Exception as e:
        print(f"In ${amt:5} USDC -> Error: {e}")

print("\n--- Testing WETH -> cbETH 1-bps (fee=100) ---")
for amt in [1, 5, 10, 20]:
    try:
        res = q.functions.quoteExactInputSingle({
            "tokenIn": weth,
            "tokenOut": cbeth,
            "amountIn": int(amt * 1e18),
            "fee": 100,
            "sqrtPriceLimitX96": 0
        }).call()
        out = res[0] / 1e18
        print(f"In {amt:2} WETH -> Out {out:8.4f} cbETH (rate: {out/amt:.6f})")
    except Exception as e:
        print(f"In {amt:2} WETH -> Error: {e}")
