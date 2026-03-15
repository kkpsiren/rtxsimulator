pub mod calldata_decoder;
pub mod decoder;
pub mod inspector;
pub mod rpc_simulator;
pub mod simulator;
pub mod types;

pub use types::{Effect, SimulationRequest, SimulationResult};

/// Chain IDs that use EraVM (ZkSync stack) instead of standard EVM bytecode.
/// These chains require RPC-based tracing since revm can't execute EraVM bytecode.
///
/// Add your chain ID here if it runs on the ZkSync/EraVM stack.
const ERAVM_CHAIN_IDS: &[u64] = &[
    324,   // ZkSync Era
    300,   // ZkSync Sepolia
    302,   // ZkSync Goerli
    232,   // Lens
    37111, // Lens Sepolia
    388,   // Cronos zkEVM
    48900, // Zircuit
];

/// Simulate a transaction, auto-selecting the engine based on chain type:
/// - **revm** for standard EVM chains (Ethereum, Base, Arbitrum, Optimism, Polygon, etc.)
/// - **RPC tracing** for EraVM chains (ZkSync, Lens, Cronos zkEVM, etc.)
///
/// Both engines return the same `SimulationResult` format.
pub async fn simulate(req: &SimulationRequest, rpc_url: &str) -> anyhow::Result<SimulationResult> {
    if ERAVM_CHAIN_IDS.contains(&req.chain_id) {
        rpc_simulator::simulate_rpc(req, rpc_url).await
    } else {
        simulator::simulate(req, rpc_url).await
    }
}
