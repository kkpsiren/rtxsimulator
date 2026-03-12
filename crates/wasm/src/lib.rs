use rtxsimulator::SimulationRequest;
use wasm_bindgen::prelude::*;

/// Simulate an EVM transaction.
///
/// Accepts a JSON object with: from, to, data, value, chainId, blockNumber?, gasLimit?
/// Returns a JSON object with: success, gasUsed, returnData, revertReason, effects, logs, calls
#[wasm_bindgen]
pub async fn simulate(rpc_url: &str, request_json: &str) -> Result<JsValue, JsError> {
    let req: SimulationRequest =
        serde_json::from_str(request_json).map_err(|e| JsError::new(&e.to_string()))?;

    let result = rtxsimulator::simulate(&req, rpc_url)
        .await
        .map_err(|e| JsError::new(&e.to_string()))?;

    serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
}

/// Simulate an EVM transaction from a JS object (no JSON parsing needed).
#[wasm_bindgen(js_name = simulateFromObject)]
pub async fn simulate_from_object(
    rpc_url: &str,
    request: JsValue,
) -> Result<JsValue, JsError> {
    let req: SimulationRequest =
        serde_wasm_bindgen::from_value(request).map_err(|e| JsError::new(&e.to_string()))?;

    let result = rtxsimulator::simulate(&req, rpc_url)
        .await
        .map_err(|e| JsError::new(&e.to_string()))?;

    serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
}
