use std::fmt;

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary, WasmMsg};

#[cw_serde]
pub struct Config {
    /// The owner of the Factory, the DAO responsible for all changes & usage
    /// All Execute methods are restricted to this address, the CronCat DAO
    pub owner_addr: Addr,
    /// Default empty, nominated owners must accept the nomination for ownership
    /// transfer to be safely completed.
    pub nominated_owner_addr: Option<Addr>,
}

#[cw_serde]
pub struct FactoryInstantiateMsg {
    /// Owner address of the contract,
    /// CronCat DAO will be the owner since it deploys the factory
    /// Only owner can execute messages in this contract
    /// If no owner_addr is passed sender will be used as owner address
    pub owner_addr: Option<String>,
}

#[cw_serde]
pub enum FactoryExecuteMsg {
    /// Deploys contract and saves metadata of the contract to the factory
    Deploy {
        kind: VersionKind,
        module_instantiate_info: ModuleInstantiateInfo,
    },

    /// Removes contract metadata from the factory if contract is paused or it is library contract.
    /// Last version of the contract can't get removed
    Remove {
        contract_name: String,
        version: [u8; 2],
    },

    /// Update fields of the contract metadata
    UpdateMetadata {
        contract_name: String,
        version: [u8; 2],
        changelog_url: Option<String>,
        schema: Option<String>,
    },

    /// Pass through execution for versioned contract calls
    Proxy { msg: WasmMsg },

    /// Factory owner (DAO) may submit a new owner to transfer ownership
    NominateOwner { nominated_owner_addr: String },

    /// The nominated address must accept a nomination to finalize transfer of ownership
    /// NOTE: This is the only method that can be called, that isn't from current owner_addr
    AcceptNominateOwner {},

    /// Factory owner (DAO) may revoke any/all ownership nominations at any time.
    RemoveNominateOwner {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum FactoryQueryMsg {
    /// Gets the factory's config
    #[returns[Config]]
    Config {},

    /// Gets latest contract names and metadatas of the contracts
    #[returns[Vec<EntryResponse>]]
    LatestContracts {},

    /// Gets latest version metadata of the contract
    #[returns[ContractMetadataResponse]]
    LatestContract { contract_name: String },

    /// Gets metadatas of the contract
    #[returns[Vec<ContractMetadataInfo>]]
    VersionsByContractName {
        contract_name: String,
        from_index: Option<u64>,
        limit: Option<u64>,
    },

    /// Gets list of the contract names
    #[returns[Vec<String>]]
    ContractNames {
        from_index: Option<u64>,
        limit: Option<u64>,
    },

    /// Gets all contract names and metadatas stored in factory.
    #[returns[Vec<EntryResponse>]]
    AllEntries {
        from_index: Option<u64>,
        limit: Option<u64>,
    },
}

#[cw_serde]
pub struct EntryResponse {
    pub contract_name: String,
    pub metadata: ContractMetadataInfo,
}

#[cw_serde]
pub struct ContractMetadata {
    pub kind: VersionKind,
    /// Code ID of the contract to be instantiated.
    pub code_id: u64,

    /// Truncated semver so contracts could programmatically check backward compat
    pub version: [u8; 2],

    /// git commit hash
    pub commit_id: String,

    /// proof of deployed code
    pub checksum: String,

    /// public link to a README about this version
    pub changelog_url: Option<String>,

    /// types/schema - helps keep UI/clients backward compatible
    pub schema: Option<String>,
}
#[cw_serde]
pub struct ContractMetadataInfo {
    pub kind: VersionKind,
    pub code_id: u64,
    pub contract_addr: Addr,
    pub version: [u8; 2],
    pub commit_id: String,
    pub checksum: String,
    pub changelog_url: Option<String>,
    pub schema: Option<String>,
}
#[cw_serde]
pub struct ContractMetadataResponse {
    pub metadata: Option<ContractMetadataInfo>,
}

/// Information needed to instantiate a module.
#[cw_serde]
pub struct ModuleInstantiateInfo {
    /// Code ID of the contract to be instantiated.
    pub code_id: u64,

    /// Truncated semver so contracts could programmatically check backward compat
    pub version: [u8; 2],

    /// git commit hash
    pub commit_id: String,

    /// proof of deployed code
    pub checksum: String,

    /// public link to a README about this version
    pub changelog_url: Option<String>,

    /// types/schema - helps keep UI/clients backward compatible
    pub schema: Option<String>,

    /// Instantiate message to be used to create the contract.
    pub msg: Binary,

    /// Contract name for the instantiated contract.
    pub contract_name: String,
}

#[cw_serde]
#[derive(Copy)]
pub enum VersionKind {
    Library,
    Manager,
    Tasks,
    Agents,
    // Recipes?
}

impl fmt::Display for VersionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionKind::Library => write!(f, "library"),
            VersionKind::Manager => write!(f, "manager"),
            VersionKind::Tasks => write!(f, "tasks"),
            VersionKind::Agents => write!(f, "agents"),
        }
    }
}

// Reference: https://github.com/DA0-DA0/dao-contracts/blob/fa567797e2f42e70296a2d6f889f341ff80f0695/packages/dao-interface/src/lib.rs#L17
/// Information about the CosmWasm level admin of a contract. Used in
/// conjunction with `ModuleInstantiateInfo` to instantiate modules.
#[cw_serde]
pub enum Admin {
    /// Set the admin to a specified address.
    Address { addr: String },
    /// Sets the admin as the core module address.
    CoreModule {},
}
