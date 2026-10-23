import urllib.request
import json

url = 'https://api.basescan.org/api?module=contract&action=getabi&address=0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43'
req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
try:
    with urllib.request.urlopen(req, timeout=5) as res:
        data = json.loads(res.read())
        abi_raw = data.get('result', '')
        if abi_raw.startswith('['):
            abi = json.loads(abi_raw)
            for item in abi:
                if item.get('type') == 'function' and 'swap' in item.get('name', '').lower():
                    inputs = ','.join([i['type'] for i in item.get('inputs', [])])
                    print(f"{item.get('name')}({inputs})")
        else:
            print("Response:", data)
except Exception as e:
    print('Error:', e)
