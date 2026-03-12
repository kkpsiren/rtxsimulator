# rtxsimulator

Lean EVM transaction simulator. Simulates what a transaction would do without sending it.

**Decodes:** native transfers, ERC-20/721/1155 transfers & approvals, setApprovalForAll, Permit/Permit2, internal calls, contract creations, selfdestructs, revert reasons.

## Architecture

Dual-engine, auto-selected by chain ID:

| Engine | Chains | How |
|--------|--------|-----|
| **revm** | Ethereum, Base, Arbitrum, Optimism, Polygon, etc. | Forks state via `eth_createAccessList` → parallel prefetch → in-memory EVM execution with inspector |
| **RPC tracing** | ZkSync, Lens, Cronos zkEVM, Zircuit | Parallel `eth_call` + `debug_traceCall` → calldata-level effect decoding |

The revm path gives full event log output. The RPC path decodes effects from calldata (ZkSync's callTracer doesn't include logs).

## Build targets

```
crates/core   → library (Rust)
crates/cli    → native CLI binary
crates/wasm   → WebAssembly package (JS/TS, Flutter)
```

### CLI

```bash
cargo build --release

./target/release/rtxsimulator-cli \
  --from 0x4bD23Ea1F559861096697f5612D293E43749F8f9 \
  --to 0x6bDc36E20D267Ff0dd6097799f82e78907105e2F \
  --data 0x095ea7b3000000000000000000000000... \
  --value 0 \
  --chain-id 232 \
  --rpc-url https://rpc.lens.xyz
```

Outputs JSON:

```json
{
  "success": true,
  "gas_used": 31339,
  "return_data": "0x...0001",
  "revert_reason": null,
  "effects": [
    {
      "type": "Erc20Approval",
      "token": "0x6bdc...",
      "owner": "0x4bd2...",
      "spender": "0x6e4c...",
      "value": "0x2386f26fc10000"
    }
  ],
  "logs": [...],
  "calls": [...]
}
```

`--rpc-url` can also be set via `RPC_URL` env var.

### WASM (JS/TS)

```bash
# For <script type="module"> / Deno / Node
wasm-pack build crates/wasm --target web --release

# For npm/webpack/vite bundlers
wasm-pack build crates/wasm --target bundler --release
```

Usage:

```typescript
import init, { simulate, simulateFromObject } from './pkg/rtxsimulator_wasm.js';
await init();

// From JSON string
const result = await simulate('https://rpc.lens.xyz', JSON.stringify({
  from: '0x4bD23Ea1F559861096697f5612D293E43749F8f9',
  to: '0x6bDc36E20D267Ff0dd6097799f82e78907105e2F',
  data: '0x095ea7b3...',
  value: '0x0',
  chain_id: 232,
}));

// Or from a JS object directly
const result = await simulateFromObject('https://rpc.lens.xyz', {
  from: '0x4bD23Ea1F559861096697f5612D293E43749F8f9',
  to: '0x6bDc36E20D267Ff0dd6097799f82e78907105e2F',
  data: '0x095ea7b3...',
  value: '0x0',
  chain_id: 232,
});
```

### Rust library

```toml
[dependencies]
rtxsimulator = { git = "https://github.com/kkpsiren/rtxsimulator" }
```

```rust
use rtxsimulator::{simulate, SimulationRequest};

let req = SimulationRequest {
    from: "0x4bD2...".parse()?,
    to: Some("0x6bDc...".parse()?),
    data: "0x095ea7b3...".parse()?,
    value: U256::ZERO,
    chain_id: 232,
    block_number: None,
    gas_limit: None,
};

let result = simulate(&req, "https://rpc.lens.xyz").await?;
println!("{}", result.success);       // true
println!("{:?}", result.effects);     // [Erc20Approval { ... }]
```

### Flutter

Build the bundler target and consume via JS interop in Flutter Web, or use `flutter_rust_bridge` for native mobile/desktop.

## Input

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `from` | address | yes | Sender |
| `to` | address | no | Recipient (omit for contract creation) |
| `data` | hex bytes | yes | Calldata |
| `value` | uint256 | yes | Wei to send |
| `chain_id` | u64 | yes | Chain ID |
| `block_number` | u64 | no | Block to simulate against (default: latest) |
| `gas_limit` | u64 | no | Gas limit override (default: 30M) |

## Output

| Field | Description |
|-------|-------------|
| `success` | Whether the tx would succeed |
| `gas_used` | Gas consumed |
| `return_data` | Raw return bytes |
| `revert_reason` | Decoded revert string if failed |
| `effects` | Decoded effects list (see below) |
| `logs` | Raw event logs |
| `calls` | Internal call trace |

### Effect types

- `NativeTransfer` — ETH/native token transfer
- `Erc20Transfer` — ERC-20 transfer
- `Erc20Approval` — ERC-20 approve
- `Erc721Transfer` — NFT transfer
- `Erc1155TransferSingle` / `Erc1155TransferBatch` — multi-token transfers
- `ApprovalForAll` — operator grant (ERC-721/1155)
- `Permit` — EIP-2612 permit
- `Permit2Approval` — Uniswap Permit2
- `ContractCreated` — new contract deployed
- `SelfDestruct` — contract self-destructed

## Performance

USDC `approve()` simulation (public RPC, cold start):

| Chain | Engine | Time |
|-------|--------|------|
| Lens (232) | RPC tracing | ~400ms |
| Ethereum (1) | revm | ~780ms |

Bottleneck is RPC latency. With a local node, revm execution itself is <10ms.

## WASM notes

- revm runs with pure-Rust crypto on WASM (no C FFI: `c-kzg`, `secp256k1`, `blst` are disabled)
- HTTP via browser `fetch` API (reqwest + wasm-bindgen)
- On WASM, state is fully pre-fetched before execution (`CacheDB<EmptyDB>`). On native, `AlloyDB` provides a fallback for any state the access list missed.
- WASM binary size: ~2.2MB (after `wasm-opt`)

## Stack

Rust, [revm](https://github.com/bluealloy/revm) v36, [alloy](https://github.com/alloy-rs/alloy) v1, [wasm-pack](https://rustwasm.github.io/wasm-pack/)
