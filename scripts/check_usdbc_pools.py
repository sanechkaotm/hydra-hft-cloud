from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))
q96 = 2**96

# UniV3 USDC/USDBC 1-bps: 0x06959273e9a65433de71f5a452d529544e07ddd0
# PCS3  USDC/USDBC 1-bps: 0x29ed55b18af0add137952cb3e29fb77b32fce426

uni_addr = Web3.to_checksum_address("0x06959273e9a65433de71f5a452d529544e07ddd0")
pcs_addr = Web3.to_checksum_address("0x29ed55b18af0add137952cb3e29fb77b32fce426")

s_uni = w3.eth.call({"to": uni_addr, "data": "0x3850c7bd"}).hex()
s_pcs = w3.eth.call({"to": pcs_addr, "data": "0x3850c7bd"}).hex()

t0_uni = "0x" + w3.eth.call({"to": uni_addr, "data": "0x0dfe1681"}).hex()[-40:]
t1_uni = "0x" + w3.eth.call({"to": uni_addr, "data": "0xd21220a7"}).hex()[-40:]

t0_pcs = "0x" + w3.eth.call({"to": pcs_addr, "data": "0x0dfe1681"}).hex()[-40:]
t1_pcs = "0x" + w3.eth.call({"to": pcs_addr, "data": "0xd21220a7"}).hex()[-40:]

print("UniV3 t0:", t0_uni, "t1:", t1_uni)
print("PCS3  t0:", t0_pcs, "t1:", t1_pcs)

p_uni = int(s_uni[:64], 16)
p_pcs = int(s_pcs[:64], 16)

r_uni = p_uni / q96
r_pcs = p_pcs / q96

# Check token order
# USDC: 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913
# USDBC: 0xd9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA
# Both have 6 decimals!
price_uni = r_uni * r_uni
price_pcs = r_pcs * r_pcs

print(f"UniV3 price: {price_uni:.6f}")
print(f"PCS3  price: {price_pcs:.6f}")
spread_bps = ((price_pcs - price_uni) / price_uni) * 10000
print(f"Spread: {spread_bps:+.2f} bps")
