use alloy::providers::{Provider, ProviderBuilder};
use alloy_primitives::{Address, Bytes, U256};
use revm::bytecode::Bytecode;
use revm::context::TxEnv;
use revm::context_interface::result::{ExecutionResult, Output};
use revm::database::CacheDB;
use revm::primitives::TxKind;
use revm::state::AccountInfo;
use revm::{Context, InspectEvm, MainBuilder, MainContext};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::decoder::decode_effects;
use crate::inspector::TxInspector;
use crate::types::{Effect, SimulationRequest, SimulationResult};

/// The main simulation entry point.
///
/// 1. `eth_createAccessList` to discover all touched addresses + storage slots
/// 2. Pre-fetch all state in parallel via async provider
/// 3. Populate CacheDB with warm state
/// 4. Execute in revm — near-zero RPC calls during execution
pub async fn simulate(req: &SimulationRequest, rpc_url: &str) -> anyhow::Result<SimulationResult> {
    let provider = ProviderBuilder::new().connect(rpc_url).await?.erased();

    let block_id = match req.block_number {
        Some(n) => alloy::eips::BlockId::number(n),
        None => alloy::eips::BlockId::latest(),
    };

    let block_tag = match req.block_number {
        Some(n) => format!("{n:#x}"),
        None => "latest".to_string(),
    };

    let gas_limit = req.gas_limit.unwrap_or(30_000_000);

    // ── Phase 1: Discover state access patterns ───────────────────────
    let access_list = if req.to.is_some() {
        fetch_access_list(&provider, req, &block_tag, gas_limit).await
    } else {
        None
    };

    // ── Phase 2: Pre-fetch all state in parallel ──────────────────────
    let prefetched =
        prefetch_state_async(&provider, req.from, req.to, access_list.as_deref(), block_id).await;

    // ── Phase 3: Build CacheDB with pre-warmed state ──────────────────
    let mut cache_db = build_cache_db(&provider, block_id, &prefetched)?;

    // Insert pre-fetched accounts into cache.
    for (addr, info) in &prefetched.accounts {
        cache_db.insert_account_info(*addr, info.clone());
    }
    // Insert pre-fetched storage slots into cache.
    for ((addr, slot), value) in &prefetched.storage {
        cache_db.insert_account_storage(*addr, *slot, *value).ok();
    }

    // ── Phase 4: Build EVM + execute ──────────────────────────────────
    let inspector = TxInspector::new();
    let mut evm = Context::mainnet()
        .with_db(cache_db)
        .modify_cfg_chained(|cfg| {
            cfg.chain_id = req.chain_id;
            cfg.disable_nonce_check = true;
            cfg.disable_balance_check = true;
            cfg.disable_block_gas_limit = true;
            cfg.tx_gas_limit_cap = Some(u64::MAX);
        })
        .build_mainnet_with_inspector(inspector);

    let kind = match req.to {
        Some(to) => TxKind::Call(to),
        None => TxKind::Create,
    };

    let tx = TxEnv::builder()
        .caller(req.from)
        .kind(kind)
        .data(req.data.clone())
        .value(req.value)
        .gas_limit(gas_limit)
        .chain_id(Some(req.chain_id))
        .build()
        .map_err(|e| anyhow::anyhow!("invalid tx env: {e:?}"))?;

    let result = evm.inspect_one_tx(tx)?;

    // ── Phase 5: Collect + decode ─────────────────────────────────────
    // Move fields out of the inspector to avoid cloning.
    let inspector = evm.inspector;
    let logs = inspector.logs;
    let calls = inspector.calls;
    let selfdestructs = inspector.selfdestructs;

    let mut effects = decode_effects(&logs, &calls);

    for (contract, beneficiary, balance) in selfdestructs {
        effects.push(Effect::SelfDestruct {
            contract,
            beneficiary,
            balance,
        });
    }

    if req.to.is_none()
        && let ExecutionResult::Success {
            output: Output::Create(_, Some(addr)),
            ..
        } = &result
    {
        effects.push(Effect::ContractCreated { address: *addr });
    }

    let (success, gas_used, return_data, revert_reason) = match result {
        ExecutionResult::Success { gas, output, .. } => {
            let data = match output {
                Output::Call(d) => d,
                Output::Create(d, _) => d,
            };
            (true, gas.used(), data, None)
        }
        ExecutionResult::Revert { gas, output, .. } => {
            let reason = decode_revert_reason(&output);
            (false, gas.used(), output, Some(reason))
        }
        ExecutionResult::Halt { reason, gas, .. } => {
            (false, gas.used(), Bytes::new(), Some(format!("{reason:?}")))
        }
    };

    Ok(SimulationResult {
        success,
        gas_used,
        return_data,
        revert_reason,
        effects,
        logs,
        calls,
    })
}

// ── CacheDB construction: native uses AlloyDB fallback, WASM uses EmptyDB ──

#[cfg(not(target_arch = "wasm32"))]
fn build_cache_db(
    provider: &alloy::providers::DynProvider,
    block_id: alloy::eips::BlockId,
    _prefetched: &PrefetchedState,
) -> anyhow::Result<
    CacheDB<
        revm::database_interface::WrapDatabaseAsync<
            revm::database::AlloyDB<alloy::network::Ethereum, alloy::providers::DynProvider>,
        >,
    >,
> {
    use revm::database::AlloyDB;
    use revm::database_interface::WrapDatabaseAsync;

    let alloy_db = WrapDatabaseAsync::new(AlloyDB::new(provider.clone(), block_id))
        .ok_or_else(|| anyhow::anyhow!("failed to create async db wrapper (no tokio runtime?)"))?;
    Ok(CacheDB::new(alloy_db))
}

#[cfg(target_arch = "wasm32")]
fn build_cache_db(
    _provider: &alloy::providers::DynProvider,
    _block_id: alloy::eips::BlockId,
    _prefetched: &PrefetchedState,
) -> anyhow::Result<CacheDB<revm::database::EmptyDB>> {
    Ok(CacheDB::new(revm::database::EmptyDB::new()))
}

// ── Pre-fetched state container ───────────────────────────────────────

pub(crate) struct PrefetchedState {
    pub accounts: HashMap<Address, AccountInfo>,
    pub storage: HashMap<(Address, U256), U256>,
}

/// Fetch all accounts and storage slots in parallel using async provider calls.
async fn prefetch_state_async(
    provider: &alloy::providers::DynProvider,
    from: Address,
    to: Option<Address>,
    access_list: Option<&[AccessListEntry]>,
    block_id: alloy::eips::BlockId,
) -> PrefetchedState {
    // Collect unique addresses to fetch.
    let mut seen = HashSet::new();
    let mut addresses = Vec::new();
    for addr in std::iter::once(from)
        .chain(to)
        .chain(access_list.into_iter().flatten().map(|e| e.address))
    {
        if seen.insert(addr) {
            addresses.push(addr);
        }
    }

    // Fetch all accounts in parallel: (nonce, balance, code) per address.
    let account_futures: Vec<_> = addresses
        .iter()
        .map(|addr| {
            let provider = provider.clone();
            let addr = *addr;
            async move {
                use std::future::IntoFuture;
                let (nonce_res, balance_res, code_res) = futures::join!(
                    provider.get_transaction_count(addr).block_id(block_id).into_future(),
                    provider.get_balance(addr).block_id(block_id).into_future(),
                    provider.get_code_at(addr).block_id(block_id).into_future(),
                );
                let nonce = nonce_res.unwrap_or(0);
                let balance = balance_res.unwrap_or(U256::ZERO);
                let code = code_res.unwrap_or_default();
                let code_hash = if code.is_empty() {
                    revm::primitives::KECCAK_EMPTY
                } else {
                    alloy_primitives::keccak256(&code)
                };
                let bytecode = if code.is_empty() {
                    Bytecode::default()
                } else {
                    Bytecode::new_raw(code)
                };
                (
                    addr,
                    AccountInfo {
                        balance,
                        nonce,
                        code_hash,
                        account_id: None,
                        code: Some(bytecode),
                    },
                )
            }
        })
        .collect();

    // Collect all storage slots we need to fetch.
    let storage_queries: Vec<(Address, U256)> = access_list
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            entry
                .storage_keys
                .as_ref()
                .map(|keys| keys.iter().map(move |key| (entry.address, (*key).into())))
        })
        .flatten()
        .collect();

    // Fetch all storage slots in parallel.
    let storage_futures: Vec<_> = storage_queries
        .iter()
        .map(|(addr, slot)| {
            let provider = provider.clone();
            let addr = *addr;
            let slot = *slot;
            async move {
                let value = provider
                    .get_storage_at(addr, slot)
                    .block_id(block_id)
                    .await
                    .unwrap_or(U256::ZERO);
                ((addr, slot), value)
            }
        })
        .collect();

    // Await all concurrently.
    let (accounts_results, storage_results) = futures::join!(
        futures::future::join_all(account_futures),
        futures::future::join_all(storage_futures),
    );

    PrefetchedState {
        accounts: accounts_results.into_iter().collect(),
        storage: storage_results.into_iter().collect(),
    }
}

// ── Access list types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccessListResponse {
    access_list: Vec<AccessListEntry>,
    #[serde(rename = "gasUsed")]
    _gas_used: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccessListEntry {
    address: Address,
    storage_keys: Option<Vec<alloy_primitives::B256>>,
}

/// Call eth_createAccessList to discover which addresses + storage slots the tx touches.
async fn fetch_access_list(
    provider: &alloy::providers::DynProvider,
    req: &SimulationRequest,
    block_tag: &str,
    gas_limit: u64,
) -> Option<Vec<AccessListEntry>> {
    let call_obj = serde_json::json!({
        "from": req.from,
        "to": req.to,
        "data": format!("0x{}", alloy_primitives::hex::encode(&req.data)),
        "value": format!("{:#x}", req.value),
        "gas": format!("{gas_limit:#x}"),
        "maxFeePerGas": "0x77359400",
        "maxPriorityFeePerGas": "0x0",
    });

    let result: Result<AccessListResponse, _> = provider
        .raw_request("eth_createAccessList".into(), (call_obj, block_tag))
        .await;

    result.ok().map(|r| r.access_list)
}

/// Try to decode a revert reason from output bytes.
fn decode_revert_reason(output: &Bytes) -> String {
    if output.len() < 4 {
        return format!("0x{}", alloy_primitives::hex::encode(output));
    }

    let selector = &output[..4];

    // Error(string) — 0x08c379a0
    // Layout: selector(4) + offset(32) + length(32) + string data
    if selector == [0x08, 0xc3, 0x79, 0xa0] && output.len() >= 68 {
        let len = U256::from_be_slice(&output[36..68])
            .try_into()
            .unwrap_or(0usize);
        let str_end = (68 + len).min(output.len());
        if let Ok(s) = std::str::from_utf8(&output[68..str_end]) {
            return s.to_string();
        }
    }

    // Panic(uint256) — 0x4e487b71
    if selector == [0x4e, 0x48, 0x7b, 0x71] && output.len() >= 36 {
        let code = U256::from_be_slice(&output[4..36]);
        let desc = match code.try_into().unwrap_or(0xffu64) {
            0x00 => "generic compiler panic",
            0x01 => "assertion failed",
            0x11 => "arithmetic overflow/underflow",
            0x12 => "division by zero",
            0x21 => "invalid enum value",
            0x22 => "invalid storage encoding",
            0x31 => "pop on empty array",
            0x32 => "array index out of bounds",
            0x41 => "out of memory",
            0x51 => "call to zero-initialized function pointer",
            _ => "unknown panic code",
        };
        return format!("Panic({code}): {desc}");
    }

    format!("0x{}", alloy_primitives::hex::encode(output))
}
