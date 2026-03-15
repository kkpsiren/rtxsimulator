// Quick test: run with `node examples/test.mjs`
import { simulate } from '../crates/wasm/pkg/simulate_tx.js';

// Simulate an ERC-20 approve on Lens
const result = await simulate(
  'https://rpc.lens.xyz',
  JSON.stringify({
    from: '0x4bD23Ea1F559861096697f5612D293E43749F8f9',
    to: '0x6bDc36E20D267Ff0dd6097799f82e78907105e2F',
    data: '0x095ea7b30000000000000000000000006e4c6976623dfe514e14baa88562933a952d8e76000000000000000000000000000000000000000000000000002386f26fc10000',
    value: '0x0',
    chain_id: 232,
  })
);

console.log('Result:', JSON.stringify(result, null, 2));
