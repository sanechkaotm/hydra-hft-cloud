from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))
balancer = Web3.to_checksum_address("0xBA12222222228d8Ba445958a75a0704d566BF2C8")

tokens = {
    "USDC": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
    "USDT": "0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2",
    "WETH": "0x4200000000000000000000000000000000000006",
    "cbBTC": "0xcbB7C0000aB88B473b1f5aFd9ef808440eed33Bf",
}

balance_of_sel = "0x70a08231"
for name, addr in tokens.items():
    c_addr = Web3.to_checksum_address(addr)
    data = balance_of_sel + balancer[2:].lower().rjust(64, "0")
    raw = w3.eth.call({"to": c_addr, "data": data}).hex()
    bal = int(raw, 16) if raw else 0
    dec = 8 if name == "cbBTC" else (18 if name == "WETH" else 6)
    print(f"Balancer Vault {name:5} balance: {bal / 10**dec:,.2f}")
