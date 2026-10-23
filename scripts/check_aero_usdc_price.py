from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))
q96 = 2**96

u_pool = Web3.to_checksum_address("0xe5b5f522e98b5a2baae212d4da66b865b781db97")
t0 = "0x" + w3.eth.call({"to": u_pool, "data": "0x0dfe1681"}).hex()[-40:]
t1 = "0x" + w3.eth.call({"to": u_pool, "data": "0xd21220a7"}).hex()[-40:]
s0 = w3.eth.call({"to": u_pool, "data": "0x3850c7bd"}).hex()

p = int(s0[:64], 16) / q96
# AERO: 0x940181a94A35A4569E4529A3CDfB74e38FD98631 (18 dec)
# USDC: 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913 (6 dec)
print("token0:", t0, "token1:", t1)
if t0.lower() == "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913":
    # t0 is USDC (6 dec), t1 is AERO (18 dec)
    # price = (ratio^2) * (10^6 / 10^18) = ratio^2 * 1e-12 (AERO per USDC)
    aero_per_usdc = (p**2) * 1e-12
    usdc_per_aero = 1.0 / aero_per_usdc
else:
    # t0 is AERO (18 dec), t1 is USDC (6 dec)
    # price = (ratio^2) * (10^18 / 10^6) = ratio^2 * 1e12 (USDC per AERO)
    usdc_per_aero = (p**2) * 1e-12
print(f"UniV3 AERO/USDC price: ${usdc_per_aero:.4f}")
