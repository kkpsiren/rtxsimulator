use alloy_primitives::{Address, Bytes, U256};
use revm::interpreter::{CallInputs, CallOutcome, CallScheme, CreateInputs, CreateOutcome};
use revm::primitives::Log;
use revm::Inspector;

use crate::types::{CallType, EmittedLog, InternalCall};

/// Collects logs, internal calls, and selfdestructs during EVM execution.
#[derive(Debug, Default)]
pub struct TxInspector {
    pub logs: Vec<EmittedLog>,
    pub calls: Vec<InternalCall>,
    pub selfdestructs: Vec<(Address, Address, U256)>,
    depth: usize,
}

impl TxInspector {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<CTX, INTR: revm::interpreter::InterpreterTypes> Inspector<CTX, INTR> for TxInspector {
    fn log(&mut self, _context: &mut CTX, log: Log) {
        self.logs.push(EmittedLog {
            address: log.address,
            topics: log.topics().to_vec(),
            data: Bytes::copy_from_slice(log.data.data.as_ref()),
        });
    }

    fn call(&mut self, _context: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        let call_type = match inputs.scheme {
            CallScheme::Call => CallType::Call,
            CallScheme::StaticCall => CallType::StaticCall,
            CallScheme::DelegateCall => CallType::DelegateCall,
            CallScheme::CallCode => CallType::CallCode,
        };

        // Extract input bytes — CallInput may be SharedBuffer or Bytes.
        let input_bytes = match &inputs.input {
            revm::interpreter::CallInput::Bytes(b) => b.clone(),
            revm::interpreter::CallInput::SharedBuffer(_range) => Bytes::new(),
        };

        self.calls.push(InternalCall {
            depth: self.depth,
            caller: inputs.caller,
            target: inputs.target_address,
            value: inputs.call_value(),
            input: input_bytes,
            gas_limit: inputs.gas_limit,
            call_type,
        });

        self.depth += 1;
        None
    }

    fn call_end(&mut self, _context: &mut CTX, _inputs: &CallInputs, _outcome: &mut CallOutcome) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn create(&mut self, _context: &mut CTX, inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        self.calls.push(InternalCall {
            depth: self.depth,
            caller: inputs.caller(),
            target: Address::ZERO,
            value: inputs.value(),
            input: inputs.init_code().clone(),
            gas_limit: inputs.gas_limit(),
            call_type: CallType::Create,
        });

        self.depth += 1;
        None
    }

    fn create_end(
        &mut self,
        _context: &mut CTX,
        _inputs: &CreateInputs,
        _outcome: &mut CreateOutcome,
    ) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
        self.selfdestructs.push((contract, target, value));
    }
}
