use alloy_primitives::{Address, Bytes, U256};
use anyhow::Result;
use clap::Parser;
use rtxsimulator::{SimulationRequest, simulate};

#[derive(Parser, Debug)]
#[command(name = "rtxsimulator", about = "Simulate EVM transactions")]
struct Args {
    /// Sender address
    #[arg(long)]
    from: Address,

    /// Recipient address (omit for contract creation)
    #[arg(long)]
    to: Option<Address>,

    /// Calldata (hex, 0x-prefixed)
    #[arg(long, default_value = "0x")]
    data: String,

    /// Value in wei (decimal)
    #[arg(long, default_value = "0")]
    value: String,

    /// Chain ID
    #[arg(long, default_value = "1")]
    chain_id: u64,

    /// RPC URL
    #[arg(long, env = "RPC_URL")]
    rpc_url: String,

    /// Block number (default: latest)
    #[arg(long)]
    block: Option<u64>,

    /// Gas limit
    #[arg(long)]
    gas_limit: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let data: Bytes = args.data.parse()?;
    let value = if args.value.starts_with("0x") {
        U256::from_str_radix(args.value.trim_start_matches("0x"), 16)
            .map_err(|_| anyhow::anyhow!("invalid hex value: {}", args.value))?
    } else {
        U256::from_str_radix(&args.value, 10)
            .map_err(|_| anyhow::anyhow!("invalid decimal value: {}", args.value))?
    };

    let req = SimulationRequest {
        from: args.from,
        to: args.to,
        data,
        value,
        chain_id: args.chain_id,
        block_number: args.block,
        gas_limit: args.gas_limit,
    };

    let result = simulate(&req, &args.rpc_url).await?;

    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}
