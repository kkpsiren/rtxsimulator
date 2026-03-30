# simulate-tx

Lean EVM transaction simulator. Simulates what a transaction would do without sending it.

Works on any EVM chain. Supports both standard EVM and EraVM (ZkSync stack) chains.

**Decodes:** native transfers, ERC-20/721/1155 transfers & approvals, setApprovalForAll, Permit/Permit2, internal calls, contract creations, selfdestructs, revert reasons.

## Supported chains

| Engine | Chains | How |
|--------|--------|-----|
| **revm** | Ethereum, Base, Arbitrum, Optimism, Polygon, BSC, Avalanche, etc. | `eth_createAccessList` → parallel state prefetch → in-memory EVM execution with inspector |
| **RPC tracing** | ZkSync Era, Lens, Cronos zkEVM, Zircuit, etc. | Parallel `eth_call` + `debug_traceCall` → calldata-level effect decoding |

Engine is auto-selected by chain ID. The revm path gives full event log output. The RPC path decodes effects from calldata since EraVM's callTracer doesn't emit logs.

## Install

```bash
npm install simulate-tx
```

## Usage (JS/TS)

```typescript
import { simulate, simulateFromObject } from 'simulate-tx';

// Simulate a USDC approve on Ethereum
const result = await simulate('https://eth.llamarpc.com', JSON.stringify({
  from: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
  to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
  data: '0x095ea7b3...',
  value: '0x0',
  chain_id: 1,
}));

// Or pass a JS object directly
const result = await simulateFromObject('https://base-rpc.publicnode.com', {
  from: '0x...',
  to: '0x...',
  data: '0x...',
  value: '0x0',
  chain_id: 8453,  // Base
});

console.log(result.success);    // true
console.log(result.effects);    // [{ type: "Erc20Approval", token: "0x...", ... }]
console.log(result.gas_used);   // 55000
```

## Usage (CLI)

```bash
cargo build --release

./target/release/rtxsimulator-cli \
  --from 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045 \
  --to 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48 \
  --data 0x095ea7b3... \
  --value 0 \
  --chain-id 1 \
  --rpc-url https://eth.llamarpc.com
```

`--rpc-url` can also be set via `RPC_URL` env var.

## Usage (Rust)

```toml
[dependencies]
rtxsimulator = { git = "https://github.com/kkpsiren/rtxsimulator" }
```

```rust
use rtxsimulator::{simulate, SimulationRequest};

let req = SimulationRequest {
    from: "0xd8dA...".parse()?,
    to: Some("0xA0b8...".parse()?),
    data: "0x095ea7b3...".parse()?,
    value: U256::ZERO,
    chain_id: 1,
    block_number: None,
    gas_limit: None,
};

let result = simulate(&req, "https://eth.llamarpc.com").await?;
```

## Output

```json
{
  "success": true,
  "gas_used": 55582,
  "return_data": "0x...0001",
  "revert_reason": null,
  "effects": [
    {
      "type": "Erc20Approval",
      "token": "0xa0b8...",
      "owner": "0xd8da...",
      "spender": "0x6e4c...",
      "value": "0x2386f26fc10000"
    }
  ],
  "logs": [...],
  "calls": [...]
}
```

### Effect types

- `NativeTransfer` — ETH/native token transfer
- `Erc20Transfer` / `Erc20Approval` — ERC-20 transfer and approve
- `Erc721Transfer` — NFT transfer
- `Erc1155TransferSingle` / `Erc1155TransferBatch` — multi-token transfers
- `ApprovalForAll` — operator grant (ERC-721/1155)
- `Permit` — EIP-2612 permit
- `Permit2Approval` — Uniswap Permit2
- `ContractCreated` — new contract deployed
- `SelfDestruct` — contract self-destructed

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

## Performance

`approve()` simulation with public RPCs (cold start, no local node):

| Chain | Engine | Time |
|-------|--------|------|
| EraVM chain | RPC tracing | ~400ms |
| Ethereum | revm | ~780ms |

Bottleneck is RPC latency. With a local node, revm execution is <10ms.

## How it works

### revm path (standard EVM)

1. Call `eth_createAccessList` to discover all addresses + storage slots the tx touches
2. Fetch all account state (nonce, balance, code) and storage slots in parallel
3. Populate an in-memory `CacheDB` with the pre-fetched state
4. Execute the tx in [revm](https://github.com/bluealloy/revm) with a custom `Inspector` that collects logs, internal calls, and selfdestructs
5. Decode event logs into typed effects (Transfer, Approval, etc.)

### RPC tracing path (EraVM chains)

1. Fire `eth_call` and `debug_traceCall` concurrently
2. Walk the call trace tree, filtering out system contract noise
3. Decode effects from calldata (since EraVM's callTracer doesn't emit event logs)

## WASM

The npm package is a WebAssembly binary compiled from Rust. It runs in Node.js, Deno, and browsers.

- revm uses pure-Rust crypto on WASM (no C FFI)
- HTTP uses the browser `fetch` API via reqwest + wasm-bindgen
- Binary size: ~2.2MB (after `wasm-opt`)

Build from source:

```bash
# Node.js
wasm-pack build crates/wasm --target nodejs --release

# Browser / ESM
wasm-pack build crates/wasm --target web --release

# npm bundlers (webpack, vite)
wasm-pack build crates/wasm --target bundler --release
```

## Project structure

```
crates/core   → Rust library (revm + RPC simulator, decoders, types)
crates/cli    → Native CLI binary
crates/wasm   → WebAssembly package (npm: simulate-tx)
```

## Release

The release process is documented in [docs/release-workflow.md](docs/release-workflow.md). Follow that workflow to keep the workspace version, git tags, and published packages aligned.

## License

MIT
