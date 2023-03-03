use cosmwasm_schema::{cw_serde, QueryResponses};

use crate::types::ProposalStatusMatches;

#[cw_serde]
pub struct InstantiateMsg {
    pub version: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // Query proposal status and compare it to pre-defined status
    #[returns(mod_sdk::types::QueryResponse)]
    ProposalStatusMatches(ProposalStatusMatches),

    // Query proposals and check if there are any passed proposals
    #[returns(mod_sdk::types::QueryResponse)]
    HasPassedProposals {
        dao_address: String,
        start_after: Option<u64>,
        limit: Option<u64>,
    },

    // Query proposals and check if there are any passed proposals with Wasm::Migration message
    #[returns(mod_sdk::types::QueryResponse)]
    HasPassedProposalWithMigration {
        dao_address: String,
        start_after: Option<u64>,
        limit: Option<u64>,
    },

    // Check if the last proposal id is greater than specified value
    #[returns(mod_sdk::types::QueryResponse)]
    HasProposalsGtId { dao_address: String, value: u64 },
}
