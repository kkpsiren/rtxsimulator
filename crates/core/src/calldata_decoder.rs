use alloy_primitives::{Address, Bytes, U256};

use crate::types::Effect;

/// Known 4-byte function selectors.
const APPROVE: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3]; // approve(address,uint256)
const TRANSFER: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb]; // transfer(address,uint256)
const TRANSFER_FROM: [u8; 4] = [0x23, 0xb8, 0x72, 0xdd]; // transferFrom(address,address,uint256)
const SAFE_TRANSFER_FROM_721: [u8; 4] = [0x42, 0x84, 0x2e, 0x0e]; // safeTransferFrom(address,address,uint256)
const SAFE_TRANSFER_FROM_721_DATA: [u8; 4] = [0xb8, 0x8d, 0x4f, 0xde]; // safeTransferFrom(address,address,uint256,bytes)
const SET_APPROVAL_FOR_ALL: [u8; 4] = [0xa2, 0x2c, 0xb4, 0x65]; // setApprovalForAll(address,bool)
const SAFE_TRANSFER_FROM_1155: [u8; 4] = [0xf2, 0x42, 0x43, 0x2a]; // safeTransferFrom(address,address,uint256,uint256,bytes)
const SAFE_BATCH_TRANSFER_1155: [u8; 4] = [0x2e, 0xb2, 0xc2, 0xd6]; // safeBatchTransferFrom(...)
const PERMIT: [u8; 4] = [0xd5, 0x05, 0xac, 0xcf]; // permit(address,address,uint256,uint256,uint8,bytes32,bytes32)

/// Decode known function calls into effects from raw calldata.
/// This is a best-effort decoder for when we don't have event logs
/// (e.g., ZkSync chains where callTracer doesn't include logs).
pub fn decode_calldata_effects(
    contract: Address,
    caller: Address,
    data: &Bytes,
) -> Vec<Effect> {
    if data.len() < 4 {
        return Vec::new();
    }

    let selector: [u8; 4] = data[..4].try_into().unwrap();
    let params = &data[4..];

    match selector {
        APPROVE => decode_approve(contract, caller, params),
        TRANSFER => decode_transfer(contract, caller, params),
        TRANSFER_FROM => decode_transfer_from(contract, params),
        SAFE_TRANSFER_FROM_721 | SAFE_TRANSFER_FROM_721_DATA => {
            decode_transfer_from_721(contract, params)
        }
        SET_APPROVAL_FOR_ALL => decode_set_approval_for_all(contract, caller, params),
        SAFE_TRANSFER_FROM_1155 => decode_safe_transfer_1155(contract, params),
        PERMIT => decode_permit(contract, params),
        _ => Vec::new(),
    }
}

fn read_address(params: &[u8], offset: usize) -> Option<Address> {
    if params.len() < offset + 32 {
        return None;
    }
    Some(Address::from_slice(&params[offset + 12..offset + 32]))
}

fn read_u256(params: &[u8], offset: usize) -> Option<U256> {
    if params.len() < offset + 32 {
        return None;
    }
    Some(U256::from_be_slice(&params[offset..offset + 32]))
}

fn read_bool(params: &[u8], offset: usize) -> Option<bool> {
    read_u256(params, offset).map(|v| v != U256::ZERO)
}

// approve(address spender, uint256 value)
fn decode_approve(token: Address, owner: Address, params: &[u8]) -> Vec<Effect> {
    let Some(spender) = read_address(params, 0) else {
        return Vec::new();
    };
    let Some(value) = read_u256(params, 32) else {
        return Vec::new();
    };
    vec![Effect::Erc20Approval {
        token,
        owner,
        spender,
        value,
    }]
}

// transfer(address to, uint256 value)
fn decode_transfer(token: Address, from: Address, params: &[u8]) -> Vec<Effect> {
    let Some(to) = read_address(params, 0) else {
        return Vec::new();
    };
    let Some(value) = read_u256(params, 32) else {
        return Vec::new();
    };
    vec![Effect::Erc20Transfer {
        token,
        from,
        to,
        value,
    }]
}

// transferFrom(address from, address to, uint256 value)
fn decode_transfer_from(token: Address, params: &[u8]) -> Vec<Effect> {
    let Some(from) = read_address(params, 0) else {
        return Vec::new();
    };
    let Some(to) = read_address(params, 32) else {
        return Vec::new();
    };
    let Some(value) = read_u256(params, 64) else {
        return Vec::new();
    };
    vec![Effect::Erc20Transfer {
        token,
        from,
        to,
        value,
    }]
}

// safeTransferFrom(address from, address to, uint256 tokenId)
fn decode_transfer_from_721(token: Address, params: &[u8]) -> Vec<Effect> {
    let Some(from) = read_address(params, 0) else {
        return Vec::new();
    };
    let Some(to) = read_address(params, 32) else {
        return Vec::new();
    };
    let Some(token_id) = read_u256(params, 64) else {
        return Vec::new();
    };
    vec![Effect::Erc721Transfer {
        token,
        from,
        to,
        token_id,
    }]
}

// setApprovalForAll(address operator, bool approved)
fn decode_set_approval_for_all(
    token: Address,
    owner: Address,
    params: &[u8],
) -> Vec<Effect> {
    let Some(operator) = read_address(params, 0) else {
        return Vec::new();
    };
    let Some(approved) = read_bool(params, 32) else {
        return Vec::new();
    };
    vec![Effect::ApprovalForAll {
        token,
        owner,
        operator,
        approved,
    }]
}

// safeTransferFrom(address from, address to, uint256 id, uint256 value, bytes data)
fn decode_safe_transfer_1155(token: Address, params: &[u8]) -> Vec<Effect> {
    let Some(from) = read_address(params, 0) else {
        return Vec::new();
    };
    let Some(to) = read_address(params, 32) else {
        return Vec::new();
    };
    let Some(id) = read_u256(params, 64) else {
        return Vec::new();
    };
    let Some(value) = read_u256(params, 96) else {
        return Vec::new();
    };
    // We use Erc1155TransferSingle; operator = from since we decode from calldata
    vec![Effect::Erc1155TransferSingle {
        token,
        operator: from,
        from,
        to,
        id,
        value,
    }]
}

// permit(address owner, address spender, uint256 value, uint256 deadline, uint8 v, bytes32 r, bytes32 s)
fn decode_permit(token: Address, params: &[u8]) -> Vec<Effect> {
    let Some(owner) = read_address(params, 0) else {
        return Vec::new();
    };
    let Some(spender) = read_address(params, 32) else {
        return Vec::new();
    };
    let Some(value) = read_u256(params, 64) else {
        return Vec::new();
    };
    vec![Effect::Permit {
        token,
        owner,
        spender,
        value,
    }]
}
