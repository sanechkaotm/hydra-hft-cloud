from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))
s_uni = w3.eth.call({"to": "0xfBB6Eed8e7aa03B138556eeDaF5D271A5E1e43ef", "data": "0x3850c7bd"}).hex()
s_pcs = w3.eth.call({"to": "0xb94b22332ABf5f89877A14Cc88f2aBC48c34B3Df", "data": "0x3850c7bd"}).hex()

print("s_uni:", s_uni)
print("s_pcs:", s_pcs)
# In live_feed.rs: hex[34..66]
p_uni = int(s_uni[34:66], 16)
p_pcs = int(s_pcs[34:66], 16)
q96 = 2**96
r_uni = p_uni / q96
r_pcs = p_pcs / q96
print("p_uni:", p_uni, "r_uni:", r_uni)
print("p_pcs:", p_pcs, "r_pcs:", r_pcs)
price_uni = 100.0 / (r_uni * r_uni)
price_pcs = 100.0 / (r_pcs * r_pcs)
print(f"Price Uni: ${price_uni:,.2f}")
print(f"Price PCS: ${price_pcs:,.2f}")
diff_bps = ((price_pcs - price_uni) / price_uni) * 10000
print(f"Diff: {diff_bps:+.2f} bps")
