// Quick test: node examples/test.mjs
//
// Simulates a USDC approve on Ethereum mainnet.
// Change the RPC URL and chain_id for other chains.

import { simulate } from '../crates/wasm/pkg/simulate_tx.js';

const result = await simulate(
  'https://ethereum-rpc.publicnode.com',
  JSON.stringify({
    from: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
    to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
    data: '0x095ea7b30000000000000000000000006e4c6976623dfe514e14baa88562933a952d8e76000000000000000000000000000000000000000000000000002386f26fc10000',
    value: '0x0',
    chain_id: 1,
  })
);

console.log(JSON.stringify(result, null, 2));
