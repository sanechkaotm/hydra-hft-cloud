# SYSTEM SPECIFICATION: HYDRA-HFT v4.0 (DISTRIBUTED TRI-NODE ARBITRAGE ENGINE)
**Target Environment:** Google Antigravity Autonomous Engineering Agent  
**Language Stack:** Rust (2024 Edition, Nightly/Stable Low-Latency Profile) + Solidity (`^0.8.24`, Foundry)  
**Target Topology:** Tokyo (`ap-northeast-1`) <-> London (`eu-west-2`) <-> Frankfurt (`eu-central-1`)  

---

## 0. AGENT DIRECTIVES & STRICT CODE GENERATION RULES
You are acting as a Principal High-Frequency Trading (HFT) Systems Architect, Low-Latency Rust Core Developer, and Senior DeFi Smart Contract Auditor. Your task is to build the complete, production-ready **Hydra-HFT** monorepo from scratch.

### Strict Rules of Engagement:
1. **NO STUBS OR PLACEHOLDERS:** You are strictly forbidden from writing `// TODO`, `unimplemented!()`, `todo!()`, or empty function bodies. Every module, mathematical formula, parser, and smart contract function must be fully implemented.
2. **ZERO-ALLOCATION HOT PATH:** In the critical execution loop (Market Data Tick -> Orderbook/AMM Update -> Graph Edge Weight Recalculation -> Negative Cycle Detection -> Optimal Sizing -> Order/Tx Serialization), heap allocations (`Vec::push`, `Box::new`, `String::from`, `HashMap` resizing) are FORBIDDEN. Use pre-allocated fixed-size arrays, stack buffers, `ArrayVec`, and integer Node/Edge indices.
3. **ZERO-LOSS ATOMICITY & IOC SAFETY:**
  - On DEX (Frankfurt Node): All multi-hop swaps must execute inside an atomic smart contract (`HydraAtomicExecutor.sol`) that reverts the entire transaction if `finalBalance < initialBalance + minProfit`.
  - On CEX (Tokyo & London Nodes): All legs must use **IOC (Immediate-Or-Cancel)** orders with an automated sub-10ms `EmergencyUnwind` routine if a multi-leg cycle partially fills.
4. **PHASED VERIFICATION:** After completing each phase, you must run `cargo check`, `cargo test`, and `forge test` to mathematically prove correctness before moving to the next phase.

---

## 1. MONOREPO DIRECTORY ARCHITECTURE

Create the following exact Cargo Workspace + Foundry hybrid structure:

```text
hydra-hft/
├── Cargo.toml                        # Workspace root with aggressive release optimizations
├── rust-toolchain.toml               # Pin stable/nightly toolchain with target-cpu=native
├── .env.example                      # Template for API keys, RPC endpoints, WireGuard IPs, Risk caps
├── crates/
│   ├── hydra-core/                   # Fixed-point math, lock-free SPSC queues, Node/Edge IDs, telemetry types
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs              # CurrencyId, VenueId, FixedPoint (I64F64 / u128), OrderBookSnapshot
│   │       ├── spsc.rs               # Lock-free Single-Producer Single-Consumer ring buffer
│   │       └── fees.rs               # Exact tier-based fee & gas deduction calculators
│   ├── hydra-graph/                  # Logarithmic Bellman-Ford / SPFA & Optimal Trade Sizing Engine
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── graph.rs              # Flat-array adjacency list for 128 currencies & 2048 directed edges
│   │       ├── spfa.rs               # Modified k-hop (k=3,4) negative cycle detector (<100us execution)
│   │       └── sizing.rs             # Multi-level L2 Orderbook walker & AMM quadratic optimal input solver
│   ├── hydra-net/                    # Ultra-low-latency networking (WebSocket zero-copy + Tri-Node UDP Mesh)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── udp_mesh.rs           # 64-byte binary UDP Lead-Lag impulse broadcaster (Tokyo<->London<->Frankfurt)
│   │       └── ws_client.rs          # Tuned TCP_NODELAY WebSocket stream manager with auto-reconnect
│   ├── hydra-cex/                    # CEX Connectors & IOC Execution (Binance in Tokyo, Kraken in London)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── orderbook.rs          # Local L2 Book maintainer with sequence gap validation
│   │       ├── binance.rs            # Binance Spot/Margin WebSocket diff-depth & REST/WS IOC order sender
│   │       ├── kraken.rs             # Kraken WebSocket v2 book & execution client
│   │       └── unwind.rs             # Sub-10ms Emergency Leg-Break Unwind state machine
│   ├── hydra-dex/                    # L2 EVM Atomic Engine (Frankfurt Node: Base & Arbitrum)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── amm_math.rs           # Constant product (UniV2/Aerodrome), Concentrated Liquidity (UniV3), StableSwap (Curve)
│   │       ├── state_sync.rs         # Alloy-rs WSS block/log/sequencer feed listener
│   │       └── tx_builder.rs         # Pre-signed EIP-1559 calldata encoder for HydraAtomicExecutor.sol
│   ├── hydra-risk/                   # Military-Grade Circuit Breakers & Kill-Switch
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── killswitch.rs         # AtomicBool global halt, drawdown tracker, revert counter, latency guard
│   │       └── alerts.rs             # Asynchronous non-blocking Telegram / Webhook telemetry reporter
│   └── hydra-node/                   # Main Binary Entrypoint & CLI
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs               # Core pinning, thread spawning, CLI flags (--role, --dry-run, --live)
│           └── config.rs             # Configuration loader
├── contracts/                        # Foundry Smart Contract Suite for Node C (Frankfurt)
│   ├── foundry.toml
│   ├── src/
│   │   ├── HydraAtomicExecutor.sol   # Atomic multi-DEX router + Aave V3 / Balancer FlashLoan receiver
│   │   └── interfaces/
│   │       └── IDexRouters.sol       # Minimal interfaces for UniV3, Aerodrome, Curve, Aave V3, IERC20
│   └── test/
│       └── HydraAtomicExecutor.t.sol # Complete Foundry test suite (Profitable cycle, Revert on loss, FlashLoan)
└── deploy/
   ├── Dockerfile                    # Multi-stage ultra-slim production container
   ├── docker-compose.yml            # Node deployment orchestration
   └── sysctl-hft.conf               # Linux Kernel network & CPU low-latency tuning
```
