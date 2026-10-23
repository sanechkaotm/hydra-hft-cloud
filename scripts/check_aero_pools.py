from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))

uni_factory = Web3.to_checksum_address("0x33128a8fC17869897dcE68Ed026d694621f6FDfD")
pcs_factory = Web3.to_checksum_address("0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865")
get_pool_sel = "0x1698ee82"

aero = "0x940181a94A35A4569E4529A3CDfB74e38FD98631"
usdc = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
weth = "0x4200000000000000000000000000000000000006"

for name, tA, tB in [("AERO/USDC", aero, usdc), ("AERO/WETH", aero, weth)]:
    tA_pad = tA[2:].lower().rjust(64, "0")
    tB_pad = tB[2:].lower().rjust(64, "0")
    for fee in [100, 500, 3000, 10000]:
        fee_pad = hex(fee)[2:].rjust(64, "0")
        data = get_pool_sel + tA_pad + tB_pad + fee_pad
        u_p = w3.eth.call({"to": uni_factory, "data": data}).hex()
        p_p = w3.eth.call({"to": pcs_factory, "data": data}).hex()
        u_addr = "0x" + u_p[-40:]
        p_addr = "0x" + p_p[-40:]
        if u_addr != "0x0000000000000000000000000000000000000000":
            try:
                liq = int(w3.eth.call({"to": Web3.to_checksum_address(u_addr), "data": "0x1a686502"}).hex(), 16)
                print(f"UniV3 {name} fee={fee}: {u_addr} (liq: {liq})")
            except:
                pass
        if p_addr != "0x0000000000000000000000000000000000000000":
            try:
                liq = int(w3.eth.call({"to": Web3.to_checksum_address(p_addr), "data": "0x1a686502"}).hex(), 16)
                print(f"PCS3  {name} fee={fee}: {p_addr} (liq: {liq})")
            except:
                pass
