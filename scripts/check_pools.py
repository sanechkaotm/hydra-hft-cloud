import urllib.request, json
payload = [
    {'jsonrpc':'2.0','method':'eth_call','params':[{'to':'0x1d4dab3f27c7f656b6323c1d6ef713b48a8f72f1','data':'0x0dfe1681'},'latest'],'id':1},
    {'jsonrpc':'2.0','method':'eth_call','params':[{'to':'0x1d4dab3f27c7f656b6323c1d6ef713b48a8f72f1','data':'0xd21220a0'},'latest'],'id':2},
    {'jsonrpc':'2.0','method':'eth_call','params':[{'to':'0x21594b992f68495dd28d605834b58889d0a727c7','data':'0x0dfe1681'},'latest'],'id':3},
    {'jsonrpc':'2.0','method':'eth_call','params':[{'to':'0x21594b992f68495dd28d605834b58889d0a727c7','data':'0xd21220a0'},'latest'],'id':4},
    # Also verify BRETT
    {'jsonrpc':'2.0','method':'eth_call','params':[{'to':'0xba3f945812a83471d709bce9c3ca699a19fb46f7','data':'0x0dfe1681'},'latest'],'id':5},
    {'jsonrpc':'2.0','method':'eth_call','params':[{'to':'0xba3f945812a83471d709bce9c3ca699a19fb46f7','data':'0xd21220a0'},'latest'],'id':6},
    {'jsonrpc':'2.0','method':'eth_call','params':[{'to':'0x214356cc4aab907244a791ca9735292860490d5a','data':'0x0dfe1681'},'latest'],'id':7},
    {'jsonrpc':'2.0','method':'eth_call','params':[{'to':'0x214356cc4aab907244a791ca9735292860490d5a','data':'0xd21220a0'},'latest'],'id':8}
]
headers = {'Content-Type': 'application/json', 'User-Agent': 'Mozilla/5.0'}
req = urllib.request.Request('https://base-rpc.publicnode.com', data=json.dumps(payload).encode('utf-8'), headers=headers)
res = json.loads(urllib.request.urlopen(req).read().decode('utf-8'))
for r in res:
    if 'result' in r:
        val = '0x' + r['result'][-40:].lower()
        print(f"id={r['id']}: {val}")
    else:
        print(f"id={r.get('id')}: error: {r.get('error')}")
