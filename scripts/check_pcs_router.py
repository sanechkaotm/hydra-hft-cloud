from web3 import Web3
w3 = Web3(Web3.HTTPProvider("https://base-mainnet.g.alchemy.com/v2/alch_eXDz4rBtBZIh_lJ7RW7eA"))
router = Web3.to_checksum_address("0x1b81D678ffb9C0263b24A97847620C99d213eB14")

# Check if router code exists
code = w3.eth.get_code(router)
print("Router code size:", len(code), "bytes")

# Check selector for exactInputSingle
# IUniswapV3SwapRouter exactInputSingle: 0x414bacae
sel = "0x414bacae"
try:
    # Try calling with dummy params to see if selector exists
    # If selector does not exist it will revert with empty data or error
    res = w3.eth.call({"to": router, "data": sel + "00"*224})
    print("Call result:", res.hex())
except Exception as e:
    print("Call response:", e)
