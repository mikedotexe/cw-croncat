use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use croncat_sdk_factory::msg::ContractMetadata;
use cw_storage_plus::{Item, Map};

pub use croncat_sdk_factory::msg::Config;

/// Contract config, just the owner address for now, preferably dao
pub const CONFIG: Item<Config> = Item::new("config");

/// Contract name with the version to the metadata
pub const CONTRACT_METADATAS: Map<(&str, &[u8]), ContractMetadata> = Map::new("contract_metadatas");

/// Contract name with the version to the Addr
pub const CONTRACT_ADDRS: Map<(&str, &[u8]), Addr> = croncat_sdk_factory::state::CONTRACT_ADDRS;
/// Contract Addr linked to contract name
pub const CONTRACT_ADDRS_LOOKUP: Map<Addr, String> = Map::new("contract_addrs_lookup");

/// Latest contract name to the Addr
pub const LATEST_ADDRS: Map<&str, Addr> = Map::new("latest_addrs");

// Latest contract name to the version
pub const LATEST_VERSIONS: Map<&str, [u8; 2]> = Map::new("latest_versions");

#[cw_serde]
pub struct TempReply {
    pub contract_name: String,
}

// Temporary storing data for the reply
pub const TEMP_REPLY: Item<TempReply> = Item::new("temp_reply");

pub const MAX_URL_LENGTH: u16 = 1_000;
