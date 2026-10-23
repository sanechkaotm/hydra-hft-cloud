from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))
q96 = 2**96

u_addr = Web3.to_checksum_address("0xa9dafa443a02fbc907cb0093276b3e6f4ef02a46")
p_addr = Web3.to_checksum_address("0x257fcbae4ac6b26a02e4fc5e1a11e4174b5ce395")

s_u = w3.eth.call({"to": u_addr, "data": "0x3850c7bd"}).hex()
s_p = w3.eth.call({"to": p_addr, "data": "0x3850c7bd"}).hex()

t0_u = "0x" + w3.eth.call({"to": u_addr, "data": "0x0dfe1681"}).hex()[-40:]
t1_u = "0x" + w3.eth.call({"to": u_addr, "data": "0xd21220a7"}).hex()[-40:]

t0_p = "0x" + w3.eth.call({"to": p_addr, "data": "0x0dfe1681"}).hex()[-40:]
t1_p = "0x" + w3.eth.call({"to": p_addr, "data": "0xd21220a7"}).hex()[-40:]

p_u = int(s_u[:64], 16) / q96
p_p = int(s_p[:64], 16) / q96

# cbETH: 0x2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22
# WETH:  0x4200000000000000000000000000000000000006
print("UniV3 t0:", t0_u, "t1:", t1_u)
print("PCS3  t0:", t0_p, "t1:", t1_p)
# If t0 is WETH and t1 is cbETH:
# ratio = sqrt(cbETH / WETH)
# weth_per_cbeth = 1.0 / (ratio * ratio)
weth_per_cbeth_u = 1.0 / (p_u * p_u)
weth_per_cbeth_p = 1.0 / (p_p * p_p)

print(f"UniV3 WETH per cbETH: {weth_per_cbeth_u:.6f}")
print(f"PCS3  WETH per cbETH: {weth_per_cbeth_p:.6f}")
spread = ((weth_per_cbeth_p - weth_per_cbeth_u) / weth_per_cbeth_u) * 10000
print(f"Spread: {spread:+.2f} bps")
