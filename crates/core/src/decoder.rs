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
    let topic0 = *log.topics.first()?;
    let token = log.address;
    let topics = log.topics.iter().copied();

    match topic0 {
        t if t == Transfer::SIGNATURE_HASH => decode_transfer(token, &log.topics, &log.data),
        t if t == Approval::SIGNATURE_HASH => decode_approval(token, &log.topics, &log.data),
        t if t == TransferSingle::SIGNATURE_HASH => {
            let ev = TransferSingle::decode_raw_log(topics, &log.data).ok()?;
            Some(Effect::Erc1155TransferSingle {
                token,
                operator: ev.operator,
                from: ev.from,
                to: ev.to,
                id: ev.id,
                value: ev.value,
            })
        }
        t if t == TransferBatch::SIGNATURE_HASH => {
            let ev = TransferBatch::decode_raw_log(topics, &log.data).ok()?;
            Some(Effect::Erc1155TransferBatch {
                token,
                operator: ev.operator,
                from: ev.from,
                to: ev.to,
                ids: ev.ids,
                values: ev.values,
            })
        }
        t if t == ApprovalForAll::SIGNATURE_HASH => {
            let ev = ApprovalForAll::decode_raw_log(topics, &log.data).ok()?;
            Some(Effect::ApprovalForAll {
                token,
                owner: ev.account,
                operator: ev.operator,
                approved: ev.approved,
            })
        }
        t if t == Permit2Approval::SIGNATURE_HASH => {
            let ev = Permit2Approval::decode_raw_log(topics, &log.data).ok()?;
            Some(Effect::Permit2Approval {
                token: ev.token,
                owner: ev.owner,
                spender: ev.spender,
                amount: U256::from(ev.amount),
                expiration: U256::from(ev.expiration),
            })
        }
        _ => None,
    }
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

    // ERC-20: 3 topics (sig + from + to), >= 32 bytes data (value in first word)
    if topics.len() == 3 && data.len() >= 32 {
        let from = Address::from_word(topics[1]);
        let to = Address::from_word(topics[2]);
        let value = U256::from_be_slice(&data[..32]);
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
    if topics.len() == 3 && data.len() >= 32 {
        let owner = Address::from_word(topics[1]);
        let spender = Address::from_word(topics[2]);
        let value = U256::from_be_slice(&data[..32]);
        return Some(Effect::Erc20Approval {
            token,
            owner,
            spender,
            value,
        });
    }
    None
}
