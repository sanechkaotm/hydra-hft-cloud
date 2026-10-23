from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))
q96 = 2**96

def get_slot0_price(addr, dec0, dec1):
    c_addr = Web3.to_checksum_address(addr)
    raw = w3.eth.call({"to": c_addr, "data": "0x3850c7bd"}).hex()
    p = int(raw[:64], 16)
    ratio = p / q96
    # price of token0 in terms of token1
    price_t0_in_t1 = (ratio**2) * (10**dec0 / 10**dec1)
    return price_t0_in_t1

# 1. USDC / USDT 1-bps pools
# token0: USDC (6 dec), token1: USDT (6 dec)
p_uni_usdt = get_slot0_price("0xd56da2b74ba826f19015e6b7dd9dae1903e85da1", 6, 6)
p_pcs_usdt = get_slot0_price("0x5f07bb9fee6062e9d09a52e6d587c64bad6ba706", 6, 6)
spread_usdt_bps = ((p_pcs_usdt - p_uni_usdt) / p_uni_usdt) * 10000
print(f"USDC/USDT 1-bps: Uni={p_uni_usdt:.6f} | PCS={p_pcs_usdt:.6f} | Spread={spread_usdt_bps:+.2f} bps")

# 2. USDC / USDBC 1-bps pools
# token0: USDBC (6 dec), token1: USDC (6 dec)
p_uni_usdbc = get_slot0_price("0x06959273e9a65433de71f5a452d529544e07ddd0", 6, 6)
p_pcs_usdbc = get_slot0_price("0x29ed55b18af0add137952cb3e29fb77b32fce426", 6, 6)
spread_usdbc_bps = ((p_pcs_usdbc - p_uni_usdbc) / p_uni_usdbc) * 10000
print(f"USDC/USDBC 1-bps: Uni={p_uni_usdbc:.6f} | PCS={p_pcs_usdbc:.6f} | Spread={spread_usdbc_bps:+.2f} bps")

# 3. cbETH / WETH 1-bps pools
# token0: WETH (18 dec), token1: cbETH (18 dec)
p_uni_cbeth = get_slot0_price("0xa9dafa443a02fbc907cb0093276b3e6f4ef02a46", 18, 18)
p_pcs_cbeth = get_slot0_price("0x257fcbae4ac6b26a02e4fc5e1a11e4174b5ce395", 18, 18)
spread_cbeth_bps = ((p_pcs_cbeth - p_uni_cbeth) / p_uni_cbeth) * 10000
print(f"WETH/cbETH 1-bps: Uni={p_uni_cbeth:.6f} | PCS={p_pcs_cbeth:.6f} | Spread={spread_cbeth_bps:+.2f} bps")

# 4. cbBTC / WETH 1-bps pools
# token0: WETH (18 dec), token1: cbBTC (8 dec)
p_uni_cbbtc = get_slot0_price("0x4c1e404a013bf1917faf2dbf5e5cef3e9bafb5aa", 18, 8)
p_pcs_cbbtc = get_slot0_price("0xc211e1f853a898bd1302385ccde55f33a8c4b3f3", 18, 8)
spread_cbbtc_bps = ((p_pcs_cbbtc - p_uni_cbbtc) / p_uni_cbbtc) * 10000
print(f"WETH/cbBTC 1-bps: Uni={p_uni_cbbtc:.8f} | PCS={p_pcs_cbbtc:.8f} | Spread={spread_cbbtc_bps:+.2f} bps")
