use alloy_primitives::{Address, B256, Bytes, U256};
use alloy_sol_types::{sol, SolEvent};

use crate::types::{Effect, EmittedLog, InternalCall};

// ── ERC-20 ────────────────────────────────────────────────────────
sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);
}

// ── ERC-1155 ──────────────────────────────────────────────────────
sol! {
    event TransferSingle(
        address indexed operator,
        address indexed from,
        address indexed to,
        uint256 id,
        uint256 value
    );
    event TransferBatch(
        address indexed operator,
        address indexed from,
        address indexed to,
        uint256[] ids,
        uint256[] values
    );
    event ApprovalForAll(
        address indexed account,
        address indexed operator,
        bool approved
    );
}

// ── Permit2 ───────────────────────────────────────────────────────
sol! {
    event Permit2Approval(
        address indexed owner,
        address indexed token,
        address indexed spender,
        uint160 amount,
        uint48 expiration
    );
}

/// Decode a list of raw logs + internal calls into a clean effects list.
pub fn decode_effects(logs: &[EmittedLog], calls: &[InternalCall]) -> Vec<Effect> {
    let mut effects = Vec::new();

    // Native transfers from internal calls with value > 0.
    for call in calls {
        if call.value > U256::ZERO {
            effects.push(Effect::NativeTransfer {
                from: call.caller,
                to: call.target,
                value: call.value,
            });
        }
    }

    // Decode logs.
    for log in logs {
        if let Some(effect) = decode_log(log) {
            effects.push(effect);
        }
    }

    effects
}

fn decode_log(log: &EmittedLog) -> Option<Effect> {
    if log.topics.is_empty() {
        return None;
    }

    let topic0 = log.topics[0];
    let token = log.address;

    // Transfer — could be ERC-20 (2 indexed + data) or ERC-721 (3 indexed)
    if topic0 == Transfer::SIGNATURE_HASH {
        return decode_transfer(token, &log.topics, &log.data);
    }

    // Approval (ERC-20 or EIP-2612 permit — same event)
    if topic0 == Approval::SIGNATURE_HASH {
        return decode_approval(token, &log.topics, &log.data);
    }

    // ERC-1155 TransferSingle
    if topic0 == TransferSingle::SIGNATURE_HASH {
        if let Ok(ev) = TransferSingle::decode_raw_log(log.topics.iter().copied(), &log.data) {
            return Some(Effect::Erc1155TransferSingle {
                token,
                operator: ev.operator,
                from: ev.from,
                to: ev.to,
                id: ev.id,
                value: ev.value,
            });
        }
    }

    // ERC-1155 TransferBatch
    if topic0 == TransferBatch::SIGNATURE_HASH {
        if let Ok(ev) = TransferBatch::decode_raw_log(log.topics.iter().copied(), &log.data) {
            return Some(Effect::Erc1155TransferBatch {
                token,
                operator: ev.operator,
                from: ev.from,
                to: ev.to,
                ids: ev.ids,
                values: ev.values,
            });
        }
    }

    // ApprovalForAll
    if topic0 == ApprovalForAll::SIGNATURE_HASH {
        if let Ok(ev) = ApprovalForAll::decode_raw_log(log.topics.iter().copied(), &log.data) {
            return Some(Effect::ApprovalForAll {
                token,
                owner: ev.account,
                operator: ev.operator,
                approved: ev.approved,
            });
        }
    }

    // Permit2
    if topic0 == Permit2Approval::SIGNATURE_HASH {
        if let Ok(ev) = Permit2Approval::decode_raw_log(log.topics.iter().copied(), &log.data) {
            return Some(Effect::Permit2Approval {
                token: ev.token,
                owner: ev.owner,
                spender: ev.spender,
                amount: U256::from(ev.amount),
                expiration: U256::from(ev.expiration),
            });
        }
    }

    None
}

/// Distinguish ERC-20 Transfer (2 indexed args + uint256 data) from
/// ERC-721 Transfer (3 indexed args, no data besides token_id in topic).
fn decode_transfer(token: Address, topics: &[B256], data: &Bytes) -> Option<Effect> {
    // ERC-721: 4 topics (sig + from + to + tokenId), empty or zero data
    if topics.len() == 4 {
        let from = Address::from_word(topics[1]);
        let to = Address::from_word(topics[2]);
        let token_id = U256::from_be_bytes(topics[3].0);
        return Some(Effect::Erc721Transfer {
            token,
            from,
            to,
            token_id,
        });
    }

    // ERC-20: 3 topics (sig + from + to), 32 bytes data = value
    if topics.len() == 3 && data.len() == 32 {
        let from = Address::from_word(topics[1]);
        let to = Address::from_word(topics[2]);
        let value = U256::from_be_slice(data);
        return Some(Effect::Erc20Transfer {
            token,
            from,
            to,
            value,
        });
    }

    None
}

fn decode_approval(token: Address, topics: &[B256], data: &Bytes) -> Option<Effect> {
    if topics.len() == 3 && data.len() == 32 {
        let owner = Address::from_word(topics[1]);
        let spender = Address::from_word(topics[2]);
        let value = U256::from_be_slice(data);
        return Some(Effect::Erc20Approval {
            token,
            owner,
            spender,
            value,
        });
    }
    None
}
