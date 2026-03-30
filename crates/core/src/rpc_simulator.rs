use alloy::providers::{Provider, ProviderBuilder};
use alloy_primitives::{Address, Bytes, U256};
use serde::Deserialize;

use crate::calldata_decoder::decode_calldata_effects;
use crate::types::{CallType, Effect, EmittedLog, InternalCall, SimulationRequest, SimulationResult};

/// RPC-based simulator for EraVM chains where revm can't execute natively.
/// Uses `debug_traceCall` with callTracer + calldata decoding.
///
/// Works on any chain that supports the `debug_traceCall` JSON-RPC method.
/// eth_call and debug_traceCall run concurrently for speed.
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
    let (gas_used, calls, logs) = process_trace_result(trace_result)?;

    // Filter out ZkSync system contract noise.
    let user_calls: Vec<InternalCall> = calls
        .into_iter()
        .filter(|c| !is_system_address(&c.target))
        .collect();

    // ── Decode effects ────────────────────────────────────────────────
    let effects = collect_effects(req, &user_calls, &logs);

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

fn process_trace_result<E: std::fmt::Display>(
    trace_result: Result<CallTrace, E>,
) -> anyhow::Result<(u64, Vec<InternalCall>, Vec<EmittedLog>)> {
    let trace = trace_result.map_err(|e| anyhow::anyhow!("debug_traceCall failed: {e}"))?;
    let gas_used = u64::from_str_radix(trace.gas_used.trim_start_matches("0x"), 16).unwrap_or(0);
    let mut calls = Vec::new();
    let mut logs = Vec::new();
    flatten_trace(&trace, &mut calls, &mut logs);
    logs.retain(|log| !is_system_address(&log.address));
    Ok((gas_used, calls, logs))
}

fn collect_effects(
    _req: &SimulationRequest,
    user_calls: &[InternalCall],
    logs: &[EmittedLog],
) -> Vec<Effect> {
    let mut effects = Vec::new();
    let decode_from_calldata = logs.is_empty();

    for call in user_calls {
        if call.value > U256::ZERO {
            effects.push(Effect::NativeTransfer {
                from: call.caller,
                to: call.target,
                value: call.value,
            });
        }
        if decode_from_calldata && !call.input.is_empty() {
            effects.extend(decode_calldata_effects(call.target, call.caller, &call.input));
        }
    }

    if !logs.is_empty() {
        effects.extend(crate::decoder::decode_effects(logs, &[]));
    }

    effects
}

// ── Trace types ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallTrace {
    from: Address,
    to: Address,
    #[serde(default)]
    input: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    gas: String,
    #[serde(default)]
    gas_used: String,
    #[serde(default, rename = "type")]
    call_type: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_failures_are_returned_as_errors() {
        let err = process_trace_result(Err("trace unavailable".to_string())).unwrap_err();
        assert!(err.to_string().contains("debug_traceCall failed"));
    }

    #[test]
    fn top_level_native_transfer_is_not_duplicated() {
        let from: Address = "0x1000000000000000000000000000000000000001".parse().unwrap();
        let to: Address = "0x2000000000000000000000000000000000000002".parse().unwrap();
        let req = SimulationRequest {
            from,
            to: Some(to),
            data: "0x".parse().unwrap(),
            value: U256::from(7),
            chain_id: 324,
            block_number: None,
            gas_limit: None,
        };
        let calls = vec![InternalCall {
            depth: 0,
            caller: from,
            target: to,
            value: U256::from(7),
            input: "0x".parse().unwrap(),
            gas_limit: 21_000,
            call_type: CallType::Call,
        }];

        let effects = collect_effects(&req, &calls, &[]);

        assert_eq!(
            effects
                .iter()
                .filter(|effect| matches!(effect, Effect::NativeTransfer { .. }))
                .count(),
            1
        );
    }
}
