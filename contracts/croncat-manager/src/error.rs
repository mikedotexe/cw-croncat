use cosmwasm_std::StdError;
use croncat_sdk_manager::SdkError;
use cw_utils::ParseReplyError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Sdk(#[from] SdkError),

    #[error(transparent)]
    ParseReplyError(#[from] ParseReplyError),

    #[error("Account is either not a registered agent or is not active yet")]
    AgentNotActive {},

    #[error("No coin balance found")]
    EmptyBalance {},

    #[error("Invalid gas_price")]
    InvalidGasPrice {},

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Unauthorized method, restricted to owner or not allowed")]
    UnauthorizedMethod {},

    #[error("Invalid Pause Admin")]
    InvalidPauseAdmin,

    #[error("Contract is in paused state")]
    ContractPaused,

    #[error("Contract is in unpaused state")]
    ContractUnpaused,

    #[error("Must not attach funds of this coin denom")]
    RedundantFunds {},

    #[error(
        "Invalid attached coins. Coins are limited to native and ibc coins configured by owner"
    )]
    InvalidAttachedCoins {},

    #[error("Task balance is empty cannot continue")]
    TaskBalanceEmpty {},

    #[error("Unknown task hash")]
    NoTaskHash {},

    #[error("Invalid version key, please update it")]
    InvalidKey {},

    #[error("Agent doesn't have to do a task in this slot")]
    NoTaskForAgent {},

    #[error("No tasks to be done in this slot")]
    NoTask {},

    // Note: this should never happen unless agent_fee + treasury_fee got compromised
    #[error("Invalid gas calculation")]
    InvalidGasCalculation {},

    #[error("No rewards available for withdraw")]
    NoWithdrawRewardsAvailable {},

    #[error("No rewards owner agent found")]
    NoRewardsOwnerAgentFound {},

    #[error("Task is no longer valid")]
    TaskNoLongerValid {},

    #[error("Task is not ready yet")]
    TaskNotReady {},

    #[error("Task query result says not ready yet")]
    TaskQueryResultFalse {},

    #[error("This cw20 address is not supported")]
    NotSupportedCw20 {},

    #[error("Must provide percentage value (0-100) for field: {field}")]
    InvalidPercentage { field: String },
}
