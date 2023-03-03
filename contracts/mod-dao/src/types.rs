use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use self::dao::Status;

/// from_index: Start at the 0 index for retrieving data, unless specified for pagination
pub const DEFAULT_PAGINATION_FROM_INDEX: u64 = 0;
/// limit: will grab a total set of records or the maximum allowed.
/// 1000 because gas estimates inside DAODAO repo revealed ~4000 was gas upper limit, so we use conservative amount.
pub const DEFAULT_PAGINATION_LIMIT: u64 = 1000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ProposalStatusMatches {
    pub dao_address: String,
    pub proposal_id: u64,
    pub status: Status,
}

pub mod dao {
    use cosmwasm_std::{CosmosMsg, Empty};

    use super::*;

    #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
    #[serde(rename_all = "snake_case")]
    #[repr(u8)]
    pub enum Status {
        /// The proposal is open for voting.
        Open,
        /// The proposal has been rejected.
        Rejected,
        /// The proposal has been passed but has not been executed.
        Passed,
        /// The proposal has been passed and executed.
        Executed,
        /// The proposal has failed or expired and has been closed. A
        /// proposal deposit refund has been issued if applicable.
        Closed,
        /// The proposal has failed during execution
        ExecutionFailed,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
    #[serde(rename_all = "snake_case")]
    pub enum QueryDao {
        Proposal {
            proposal_id: u64,
        },
        ListProposals {
            start_after: Option<u64>,
            limit: Option<u64>,
        },
        ProposalCount {},
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
    pub struct ProposalResponse {
        /// The ID of the proposal being returned.
        pub id: u64,
        pub proposal: AnyChoiceProposal,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
    pub struct ProposalListResponse {
        pub proposals: Vec<ProposalResponse>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
    pub struct AnyChoiceProposal {
        pub status: Status,
        //Ignore rest
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
    pub struct SingleProposalResponse {
        /// The ID of the proposal being returned.
        pub id: u64,
        pub proposal: SingleChoiceProposal,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
    pub struct SingleProposalListResponse {
        pub proposals: Vec<SingleProposalResponse>,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
    pub struct SingleChoiceProposal {
        pub status: Status,
        pub msgs: Vec<CosmosMsg<Empty>>,
        //Ignore rest
    }
}
