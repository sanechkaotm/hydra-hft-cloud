from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))
v_pool = Web3.to_checksum_address("0x1d4dab3f27c7f656b6323c1d6ef713b48a8f72f1")
fee_hex = w3.eth.call({"to": v_pool, "data": "0xddca3f43"}).hex()
fee = int(fee_hex, 16)
print("UniV3 WETH/VIRTUAL pool fee:", fee)
