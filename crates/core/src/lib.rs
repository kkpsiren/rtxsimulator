pub mod calldata_decoder;
pub mod decoder;
pub mod inspector;
pub mod rpc_simulator;
pub mod simulator;
pub mod types;

pub use types::{Effect, SimulationRequest, SimulationResult};

/// Known ZkSync-based chain IDs where revm can't execute natively.
const ZKSYNC_CHAINS: &[u64] = &[
    324,   // ZkSync Era Mainnet
    300,   // ZkSync Sepolia
    302,   // ZkSync Goerli
    232,   // Lens Network
    37111, // Lens Sepolia
    388,   // Cronos zkEVM
    48900, // Zircuit
];

/// Simulate a transaction, auto-selecting the engine:
/// - revm for standard EVM chains
/// - RPC-based tracing for ZkSync/EraVM chains
pub async fn simulate(req: &SimulationRequest, rpc_url: &str) -> anyhow::Result<SimulationResult> {
    if ZKSYNC_CHAINS.contains(&req.chain_id) {
        rpc_simulator::simulate_rpc(req, rpc_url).await
    } else {
        simulator::simulate(req, rpc_url).await
    }
}
