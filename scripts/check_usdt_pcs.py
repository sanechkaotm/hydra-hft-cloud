from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))
q96 = 2**96

s_uni = w3.eth.call({"to": Web3.to_checksum_address("0xd56da2b74ba826f19015e6b7dd9dae1903e85da1"), "data": "0x3850c7bd"}).hex()
s_pcs = w3.eth.call({"to": Web3.to_checksum_address("0x5f07bb9fee6062e9d09a52e6d587c64bad6ba706"), "data": "0x3850c7bd"}).hex()

p_uni = int(s_uni[:64], 16)
p_pcs = int(s_pcs[:64], 16)

r_uni = p_uni / q96
r_pcs = p_pcs / q96

price_uni = r_uni * r_uni
price_pcs = r_pcs * r_pcs

print(f"UniV3 USDT per USDC: {price_uni:.6f}")
print(f"PCS3  USDT per USDC: {price_pcs:.6f}")
spread_bps = ((price_pcs - price_uni) / price_uni) * 10000
print(f"Spread: {spread_bps:+.2f} bps")
