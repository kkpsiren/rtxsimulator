use alloy::providers::{Provider, ProviderBuilder};
use alloy_primitives::{Address, Bytes, U256};
use serde::Deserialize;

use crate::calldata_decoder::decode_calldata_effects;
use crate::types::{CallType, Effect, EmittedLog, InternalCall, SimulationRequest, SimulationResult};

/// RPC-based simulator for chains where revm can't run natively (ZkSync, Lens, etc.).
/// Uses `debug_traceCall` with callTracer + calldata decoding.
///
/// Optimization: eth_call and debug_traceCall run concurrently.
pub async fn simulate_rpc(
    req: &SimulationRequest,
    rpc_url: &str,
) -> anyhow::Result<SimulationResult> {
    let provider = ProviderBuilder::new().connect(rpc_url).await?.erased();

    let call_request = serde_json::json!({
        "from": req.from,
        "to": req.to,
        "data": format!("0x{}", alloy_primitives::hex::encode(&req.data)),
        "value": format!("{:#x}", req.value),
    });
    let block_tag = match req.block_number {
        Some(n) => format!("{n:#x}"),
        None => "latest".to_string(),
    };

    // ── Fire both RPC calls concurrently ──────────────────────────────
    let trace_opts = serde_json::json!({
        "tracer": "callTracer",
        "tracerConfig": {
            "onlyTopCall": false,
            "withLog": true,
        }
    });

    let eth_call_fut = provider.raw_request::<_, String>(
        "eth_call".into(),
        (call_request.clone(), block_tag.clone()),
    );
    let trace_fut = provider.raw_request::<_, CallTrace>(
        "debug_traceCall".into(),
        (call_request, block_tag, trace_opts),
    );

    let (eth_call_result, trace_result) = futures::join!(eth_call_fut, trace_fut);

    // ── Process eth_call result ───────────────────────────────────────
    let (success, return_data, revert_reason) = match eth_call_result {
        Ok(hex_str) => {
            let bytes: Bytes = hex_str.parse().unwrap_or_default();
            (true, bytes, None)
        }
        Err(e) => {
            let reason = extract_rpc_revert_reason(&e.to_string());
            (false, Bytes::new(), Some(reason))
        }
    };

    // ── Process trace result ──────────────────────────────────────────
    let mut calls = Vec::new();
    let mut logs = Vec::new();
    let mut gas_used = 0u64;

    if let Ok(trace) = trace_result {
        gas_used = u64::from_str_radix(trace.gas_used.trim_start_matches("0x"), 16).unwrap_or(0);
        flatten_trace(&trace, &mut calls, &mut logs);
    }

    // Filter out ZkSync system contract noise.
    let user_calls: Vec<InternalCall> = calls
        .into_iter()
        .filter(|c| !is_system_address(&c.target))
        .collect();

    // ── Decode effects ────────────────────────────────────────────────
    let mut effects = Vec::new();

    if let Some(to) = req.to {
        if req.value > U256::ZERO {
            effects.push(Effect::NativeTransfer {
                from: req.from,
                to,
                value: req.value,
            });
        }
        effects.extend(decode_calldata_effects(to, req.from, &req.data));
    }

    let top_data = &req.data;
    for call in &user_calls {
        if call.value > U256::ZERO && !is_system_address(&call.target) {
            effects.push(Effect::NativeTransfer {
                from: call.caller,
                to: call.target,
                value: call.value,
            });
        }
        if call.input != *top_data && !is_system_address(&call.target) {
            effects.extend(decode_calldata_effects(call.target, call.caller, &call.input));
        }
    }

    if !logs.is_empty() {
        let log_effects = crate::decoder::decode_effects(&logs, &[]);
        effects.extend(log_effects);
    }

    Ok(SimulationResult {
        success,
        gas_used,
        return_data,
        revert_reason,
        effects,
        logs,
        calls: user_calls,
    })
}

// ── Trace types ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CallTrace {
    from: Address,
    to: Address,
    #[serde(default)]
    input: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    gas: String,
    #[serde(default)]
    gas_used: String,
    #[serde(default, rename = "type")]
    call_type: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    revert_reason: Option<String>,
    #[serde(default)]
    calls: Vec<CallTrace>,
    #[serde(default)]
    logs: Vec<TraceLog>,
}

#[derive(Debug, Deserialize)]
struct TraceLog {
    address: Address,
    topics: Vec<alloy_primitives::B256>,
    data: String,
}

// ── Helpers ───────────────────────────────────────────────────────────

fn flatten_trace(
    trace: &CallTrace,
    calls: &mut Vec<InternalCall>,
    logs: &mut Vec<EmittedLog>,
) {
    flatten_trace_inner(trace, calls, logs, 0);
}

fn flatten_trace_inner(
    trace: &CallTrace,
    calls: &mut Vec<InternalCall>,
    logs: &mut Vec<EmittedLog>,
    depth: usize,
) {
    let value = parse_hex_u256(&trace.value);
    let input: Bytes = trace.input.parse().unwrap_or_default();

    let call_type = match trace.call_type.as_str() {
        "call" | "CALL" => CallType::Call,
        "staticcall" | "STATICCALL" => CallType::StaticCall,
        "delegatecall" | "DELEGATECALL" => CallType::DelegateCall,
        "callcode" | "CALLCODE" => CallType::CallCode,
        "create" | "CREATE" => CallType::Create,
        "create2" | "CREATE2" => CallType::Create2,
        _ => CallType::Call,
    };

    let gas_limit = u64::from_str_radix(trace.gas.trim_start_matches("0x"), 16).unwrap_or(0);

    calls.push(InternalCall {
        depth,
        caller: trace.from,
        target: trace.to,
        value,
        input,
        gas_limit,
        call_type,
    });

    for log in &trace.logs {
        let data: Bytes = log.data.parse().unwrap_or_default();
        logs.push(EmittedLog {
            address: log.address,
            topics: log.topics.clone(),
            data,
        });
    }

    for child in &trace.calls {
        flatten_trace_inner(child, calls, logs, depth + 1);
    }
}

fn is_system_address(addr: &Address) -> bool {
    let bytes = addr.as_slice();
    bytes[..18] == [0u8; 18]
}

fn parse_hex_u256(s: &str) -> U256 {
    if s.is_empty() || s == "0x0" || s == "0x" {
        return U256::ZERO;
    }
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    U256::from_str_radix(stripped, 16).unwrap_or(U256::ZERO)
}

fn extract_rpc_revert_reason(error_msg: &str) -> String {
    if let Some(start) = error_msg.find("execution reverted: ") {
        return error_msg[start + 20..].to_string();
    }
    if let Some(start) = error_msg.find("0x") {
        let hex_part = &error_msg[start..];
        let end = hex_part
            .find(|c: char| !c.is_ascii_hexdigit() && c != 'x')
            .unwrap_or(hex_part.len());
        return hex_part[..end].to_string();
    }
    error_msg.to_string()
}
