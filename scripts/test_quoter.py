from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))

# Uniswap V3 QuoterV2 on Base: 0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a
quoter = Web3.to_checksum_address("0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a")
weth = Web3.to_checksum_address("0x4200000000000000000000000000000000000006")
cbbtc = Web3.to_checksum_address("0xcbB7C0000aB88B473b1f5aFd9ef808440eed33Bf")

# quoteExactInputSingle params:
# (address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, uint160 sqrtPriceLimitX96)
# selector: 0xc6a50263
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

for fee in [100, 500]:
    for amt_weth in [0.1, 0.5, 1.0, 2.0]:
        amt_wei = int(amt_weth * 1e18)
        try:
            res = q.functions.quoteExactInputSingle({
                "tokenIn": weth,
                "tokenOut": cbbtc,
                "amountIn": amt_wei,
                "fee": fee,
                "sqrtPriceLimitX96": 0
            }).call()
            out_cbbtc = res[0] / 1e8
            rate = out_cbbtc / amt_weth
            print(f"Fee {fee:3}: In {amt_weth:.1f} WETH -> Out {out_cbbtc:.6f} cbBTC (rate: {rate:.6f} cbBTC/WETH)")
        except Exception as e:
            print(f"Fee {fee:3}: In {amt_weth:.1f} WETH -> Error: {e}")
