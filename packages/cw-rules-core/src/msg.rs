use crate::types::{
    CheckOwnerOfNft, CheckPassedProposals, CheckProposalStatus, CroncatQuery, HasBalanceGte,
};
use generic_query::GenericQuery;
//use cw_croncat_core::types::Rule;
//use cosmwasm_std::Coin;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smart_query::SmartQueryHead;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    QueryResult {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // Individual query evaluations
    GetBalance {
        address: String,
        denom: String,
    },
    GetCw20Balance {
        cw20_contract: String,
        address: String,
    },
    HasBalanceGte(HasBalanceGte),
    CheckOwnerOfNft(CheckOwnerOfNft),
    CheckProposalStatus(CheckProposalStatus),
    CheckPassedProposals(CheckPassedProposals),
    GenericQuery(GenericQuery),
    // Full evaluations
    QueryConstruct(QueryConstruct),
    SmartQuery(SmartQueryHead),
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct QueryMultiResponse {
    pub data: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct QueryResponse<T = cosmwasm_std::Binary> {
    pub result: bool,
    pub data: T,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct QueryConstructResponse {
    pub result: bool,
    pub data: Vec<cosmwasm_std::Binary>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct QueryConstruct {
    pub queries: Vec<CroncatQuery>,
}
