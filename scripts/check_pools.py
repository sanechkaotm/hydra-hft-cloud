from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))

pools = [
    ("UniV3 cbBTC/USDC 5bp", "0xfbb6eed8e7aa03b138556eedaf5d271a5e1e43ef"),
    ("PCS3 cbBTC/USDC 1bp", "0xb94b22332abf5f89877a14cc88f2abc48c34b3df"),
    ("UniV3 cbBTC/WETH 5bp", "0x7aea2e8a3843516afa07293a10ac8e49906dabd1"),
    ("PCS3 cbBTC/WETH 1bp", "0xc211e1f853a898bd1302385ccde55f33a8c4b3f3"),
]

q96 = 2**96

for name, addr in pools:
    c_addr = Web3.to_checksum_address(addr)
    t0 = "0x" + w3.eth.call({"to": c_addr, "data": "0x0dfe1681"}).hex()[-40:]
    t1 = "0x" + w3.eth.call({"to": c_addr, "data": "0xd21220a7"}).hex()[-40:]
    slot0_raw = w3.eth.call({"to": c_addr, "data": "0x3850c7bd"}).hex()
    sqrt_p = int(slot0_raw[2:66], 16)
    ratio = sqrt_p / q96
    print(f"\n{name} ({c_addr}):")
    print(f"  t0: {t0}")
    print(f"  t1: {t1}")
    print(f"  sqrt_p: {sqrt_p}, ratio: {ratio}")
    # cbBTC: 0xcbb7c0000ab88b473b1f5afd9ef808440eed33bf (8 dec)
    # USDC:  0x833589fcd6edb6e08f4c7c32d4f71b54bda02913 (6 dec)
    # WETH:  0x4200000000000000000000000000000000000006 (18 dec)
    if "USDC" in name:
        if t0.lower() == "0xcbb7c0000ab88b473b1f5afd9ef808440eed33bf":
            # t0 is cbBTC, t1 is USDC
            price = (ratio**2) * (10**8 / 10**6)
        else:
            # t0 is USDC, t1 is cbBTC
            price = (1.0 / (ratio**2)) * (10**8 / 10**6)
        print(f"  Calculated USDC per cbBTC: {price:,.2f}")
    if "WETH" in name:
        if t0.lower() == "0xcbb7c0000ab88b473b1f5afd9ef808440eed33bf":
            # t0 is cbBTC, t1 is WETH
            price = (ratio**2) * (10**8 / 10**18)
        else:
            # t0 is WETH, t1 is cbBTC
            price = (1.0 / (ratio**2)) * (10**8 / 10**18)
        print(f"  Calculated WETH per cbBTC: {price:,.4f}")
