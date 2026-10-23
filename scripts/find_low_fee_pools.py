from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))

uni_factory = Web3.to_checksum_address("0x33128a8fC17869897dcE68Ed026d694621f6FDfD")
pcs_factory = Web3.to_checksum_address("0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865")

get_pool_sel = "0x1698ee82"

tokens = {
    "USDC": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
    "USDT": "0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2",
    "EURC": "0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42",
    "USDBC": "0xd9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA",
    "cbBTC": "0xcbB7C0000aB88B473b1f5aFd9ef808440eed33Bf",
    "WETH": "0x4200000000000000000000000000000000000006",
    "cbETH": "0x2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22",
}

def get_pool(factory, tA, tB, fee):
    tA_pad = tA[2:].lower().rjust(64, "0")
    tB_pad = tB[2:].lower().rjust(64, "0")
    fee_pad = hex(fee)[2:].rjust(64, "0")
    data = get_pool_sel + tA_pad + tB_pad + fee_pad
    try:
        res = w3.eth.call({"to": factory, "data": data}).hex()
        addr = "0x" + res[-40:]
        return addr
    except Exception as e:
        return "0x0000000000000000000000000000000000000000"

pairs = [
    ("USDC", "USDT"),
    ("USDC", "USDBC"),
    ("EURC", "USDC"),
    ("cbBTC", "USDC"),
    ("cbBTC", "WETH"),
    ("cbETH", "WETH"),
    ("WETH", "USDC"),
]

for nameA, nameB in pairs:
    tA = tokens[nameA]
    tB = tokens[nameB]
    for fee in [100, 500]:
        u_pool = get_pool(uni_factory, tA, tB, fee)
        p_pool = get_pool(pcs_factory, tA, tB, fee)
        if u_pool != "0x0000000000000000000000000000000000000000":
            # Check liquidity
            try:
                liq_hex = w3.eth.call({"to": Web3.to_checksum_address(u_pool), "data": "0x1a686502"}).hex()
                liq = int(liq_hex, 16) if liq_hex else 0
                print(f"UniV3 {nameA:5}/{nameB:5} fee={fee:3} -> {u_pool} (liq: {liq})")
            except:
                print(f"UniV3 {nameA:5}/{nameB:5} fee={fee:3} -> {u_pool}")
        if p_pool != "0x0000000000000000000000000000000000000000":
            try:
                liq_hex = w3.eth.call({"to": Web3.to_checksum_address(p_pool), "data": "0x1a686502"}).hex()
                liq = int(liq_hex, 16) if liq_hex else 0
                print(f"PCS3  {nameA:5}/{nameB:5} fee={fee:3} -> {p_pool} (liq: {liq})")
            except:
                print(f"PCS3  {nameA:5}/{nameB:5} fee={fee:3} -> {p_pool}")
