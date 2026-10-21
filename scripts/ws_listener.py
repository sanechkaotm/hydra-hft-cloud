#!/usr/bin/env python3
"""
HYDRA-HFT v4.1 — Sub-Second WebSocket Sequencer Event Streamer
Subscribes to Base Mainnet `newHeads` event streams via low-latency WebSockets.
On every new block / pre-confirmation header, instantly fires a microsecond UDP trigger
to `127.0.0.1:8766`, allowing `hydra-node` to wake up reactively within 0.1 ms of new state!
"""

import asyncio
import json
import socket
import sys
import time

WS_ENDPOINTS = [
    "wss://base-rpc.publicnode.com",
    "wss://base.drpc.org",
    "wss://base.gateway.tenderly.co"
]

UDP_TARGET = ("127.0.0.1", 8766)

async def run_listener():
    udp_sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    
    endpoint_idx = 0
    while True:
        url = WS_ENDPOINTS[endpoint_idx % len(WS_ENDPOINTS)]
        endpoint_idx += 1
        
        try:
            import websockets
            print(f"[*] [WS STREAM] Connecting to low-latency Base WebSocket: {url}...", flush=True)
            async with websockets.connect(url, ping_interval=20, ping_timeout=10) as ws:
                # Subscribe to new heads
                sub_payload = {"jsonrpc": "2.0", "id": 1, "method": "eth_subscribe", "params": ["newHeads"]}
                await ws.send(json.dumps(sub_payload))
                sub_ack = await asyncio.wait_for(ws.recv(), timeout=5)
                print(f"[+] [WS STREAM] Successfully subscribed to Base newHeads! SubID: {sub_ack[:40]}", flush=True)
                
                while True:
                    msg = await ws.recv()
                    head_data = json.loads(msg)
                    params = head_data.get("params", {})
                    result = params.get("result", {})
                    block_num = result.get("number", "latest")
                    base_fee = result.get("baseFeePerGas", "0x0")
                    
                    # Send instant reactive wake-up trigger to hydra-node
                    pulse_payload = f"BLOCK:{block_num}:{base_fee}".encode("utf-8")
                    udp_sock.sendto(pulse_payload, UDP_TARGET)
                    
                    ts = time.strftime("%H:%M:%S")
                    print(f"[{ts}] [WS STREAM] Block {block_num} received -> Fired instant reactive trigger to hydra-node", flush=True)
                    
        except Exception as e:
            print(f"[~] [WS STREAM] Connection issue with {url} ({e}). Reconnecting in 2s...", flush=True)
            await asyncio.sleep(2)

if __name__ == "__main__":
    try:
        asyncio.run(run_listener())
    except KeyboardInterrupt:
        print("\n[!] WS Streamer stopped.")
