use alloy_primitives::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};

/// Input to the simulator — everything needed to describe a tx.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationRequest {
    pub from: Address,
    pub to: Option<Address>,
    pub data: Bytes,
    pub value: U256,
    pub chain_id: u64,
    /// Optional block number to simulate against (defaults to latest).
    #[serde(default)]
    pub block_number: Option<u64>,
    /// Optional gas limit override.
    #[serde(default)]
    pub gas_limit: Option<u64>,
}

/// Full result of a simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub success: bool,
    pub gas_used: u64,
    pub return_data: Bytes,
    /// If the tx reverted, the decoded reason (if possible).
    pub revert_reason: Option<String>,
    /// All decoded effects the tx would produce.
    pub effects: Vec<Effect>,
    /// Raw logs emitted.
    pub logs: Vec<EmittedLog>,
    /// Internal calls traced.
    pub calls: Vec<InternalCall>,
}

/// A decoded, human-readable effect of the transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Effect {
    NativeTransfer {
        from: Address,
        to: Address,
        value: U256,
    },
    Erc20Transfer {
        token: Address,
        from: Address,
        to: Address,
        value: U256,
    },
    Erc20Approval {
        token: Address,
        owner: Address,
        spender: Address,
        value: U256,
    },
    Erc721Transfer {
        token: Address,
        from: Address,
        to: Address,
        token_id: U256,
    },
    Erc1155TransferSingle {
        token: Address,
        operator: Address,
        from: Address,
        to: Address,
        id: U256,
        value: U256,
    },
    Erc1155TransferBatch {
        token: Address,
        operator: Address,
        from: Address,
        to: Address,
        ids: Vec<U256>,
        values: Vec<U256>,
    },
    ApprovalForAll {
        token: Address,
        owner: Address,
        operator: Address,
        approved: bool,
    },
    Permit {
        token: Address,
        owner: Address,
        spender: Address,
        value: U256,
    },
    Permit2Approval {
        token: Address,
        owner: Address,
        spender: Address,
        amount: U256,
        expiration: U256,
    },
    ContractCreated {
        address: Address,
    },
    SelfDestruct {
        contract: Address,
        beneficiary: Address,
        balance: U256,
    },
}

/// Raw log emitted during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmittedLog {
    pub address: Address,
    pub topics: Vec<alloy_primitives::B256>,
    pub data: Bytes,
}

/// An internal call observed by the inspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalCall {
    pub depth: usize,
    pub caller: Address,
    pub target: Address,
    pub value: U256,
    pub input: Bytes,
    pub gas_limit: u64,
    pub call_type: CallType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallType {
    Call,
    StaticCall,
    DelegateCall,
    CallCode,
    Create,
    Create2,
}
