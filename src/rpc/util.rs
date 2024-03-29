use super::{BlockInfo, RpcError};
use crate::{Account, Block, BlockType};
use hex::FromHexError;

pub mod parse {
    pub use super::super::parse::*;
}
pub mod encode {
    pub use super::super::encode::*;
}
pub use serde_json::{Map, Value as JsonValue};

pub fn trim_json(value: &str) -> &str {
    value.trim_matches('\"')
}

pub fn from_hex(encoded: &str) -> Result<Vec<u8>, RpcError> {
    Ok(hex::decode(trim_json(encoded))?)
}

pub fn to_uppercase_hex(bytes: &[u8]) -> String {
    hex::encode(bytes).to_uppercase()
}

/// Get the keys in a Json map.
pub fn map_keys_from_json(value: &JsonValue) -> Result<Vec<&String>, RpcError> {
    Ok(value
        .as_object()
        .ok_or(RpcError::InvalidJsonDataType)?
        .keys()
        .collect())
}

pub fn usize_from_json(value: &JsonValue) -> Result<usize, RpcError> {
    trim_json(&value.to_string())
        .parse::<usize>()
        .map_err(|_| RpcError::InvalidInteger)
}

pub fn u64_from_json(value: &JsonValue) -> Result<u64, RpcError> {
    trim_json(&value.to_string())
        .parse::<u64>()
        .map_err(|_| RpcError::InvalidInteger)
}

pub fn u128_from_json(value: &JsonValue) -> Result<u128, RpcError> {
    trim_json(&value.to_string())
        .parse::<u128>()
        .map_err(|_| RpcError::InvalidInteger)
}

pub fn bool_from_json(value: &JsonValue) -> Result<bool, RpcError> {
    trim_json(&value.to_string())
        .parse::<bool>()
        .map_err(|_| RpcError::InvalidInteger)
}

pub fn bytes_from_json<const T: usize>(value: &JsonValue) -> Result<[u8; T], RpcError> {
    from_hex(&value.to_string())?
        .try_into()
        .or(Err(FromHexError::InvalidStringLength.into()))
}

pub fn block_info_from_json(value: &JsonValue, block: Block) -> Result<BlockInfo, RpcError> {
    Ok(BlockInfo {
        height: usize_from_json(&value["height"])?,
        timestamp: u64_from_json(&value["local_timestamp"])?,
        confirmed: bool_from_json(&value["confirmed"])?,
        block,
    })
}

pub fn account_from_json(value: &JsonValue) -> Result<Account, RpcError> {
    Account::try_from(trim_json(&value.to_string())).map_err(|_| RpcError::InvalidAccount)
}

pub fn block_from_json(block: &JsonValue, block_type: BlockType) -> Result<Block, RpcError> {
    Ok(Block {
        block_type,
        account: account_from_json(&block["account"])?,
        previous: bytes_from_json(&block["previous"])?,
        representative: account_from_json(&block["representative"])?,
        balance: u128_from_json(&block["balance"])?,
        link: bytes_from_json(&block["link"])?,
        signature: bytes_from_json::<64>(&block["signature"])?
            .try_into()
            .unwrap(),
        work: bytes_from_json(&block["work"])?,
    })
}

/// Specific to `account_history`
pub(crate) fn block_from_history_json(block: &JsonValue) -> Result<Block, RpcError> {
    let block_type = block["type"].to_string();
    let block_type = trim_json(&block_type);
    let block_type = if block_type == "state" {
        // state blocks
        BlockType::from_subtype_string(trim_json(&block["subtype"].to_string()))
    } else {
        // legacy blocks (shouldn't be needed)
        Some(BlockType::Legacy(block_type.to_string()))
    };

    block_from_json(block, block_type.ok_or(RpcError::InvalidJsonDataType)?)
}

/// Specific to `block_info` and `blocks_info`
pub(crate) fn block_from_info_json(block: &JsonValue) -> Result<Block, RpcError> {
    let contents = &block["contents"];
    let block_type = contents["type"].to_string();
    let block_type = trim_json(&block_type);
    let block_type = if block_type == "state" {
        // state blocks
        BlockType::from_subtype_string(trim_json(&block["subtype"].to_string()))
    } else {
        // legacy blocks (shouldn't be needed)
        Some(BlockType::Legacy(block_type.to_string()))
    };

    block_from_json(contents, block_type.ok_or(RpcError::InvalidJsonDataType)?)
}

/// **Does not handle "subtype" field**
pub fn block_to_json(block: &Block) -> Map<String, JsonValue> {
    let block_type: &str = match &block.block_type {
        BlockType::Legacy(block_type) => block_type,
        _ => "state",
    };

    let mut json_block = Map::new();
    json_block.insert("type".into(), block_type.into());
    json_block.insert("account".into(), block.account.clone().into());
    json_block.insert("previous".into(), to_uppercase_hex(&block.previous).into());
    json_block.insert("representative".into(), block.representative.clone().into());
    json_block.insert("balance".into(), block.balance.to_string().into());
    json_block.insert("link".into(), to_uppercase_hex(&block.link).into());
    json_block.insert(
        "signature".into(),
        to_uppercase_hex(&block.signature.to_bytes()).into(),
    );
    json_block.insert("work".into(), hex::encode(block.work).into());
    json_block
}

/// Sanity check to ensure that no overflow occurs
pub fn balances_sanity_check(blocks: &[Block]) -> Result<(), RpcError> {
    let mut total: u128 = 0;
    let mut overflow: bool;
    for block in blocks {
        (total, overflow) = total.overflowing_add(block.balance);
        if overflow {
            return Err(RpcError::InvalidData);
        }
    }
    Ok(())
}
