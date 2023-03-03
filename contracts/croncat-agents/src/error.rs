use cosmwasm_std::{Coin, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    CoreError(#[from] croncat_sdk_agents::error::CoreError),

    #[error("Agent already registered")]
    AgentAlreadyRegistered,

    #[error("Agent not registered")]
    AgentNotRegistered,

    #[error("Agent is not active")]
    AgentNotActive,

    #[error("Agent is not in pending set")]
    AgentNotPending,

    #[error("Insufficient funds. Need a balance of at least {amount_needed:?} to cover the first few task chain fees")]
    InsufficientFunds { amount_needed: Coin },

    #[error("Contract is in paused state")]
    ContractPaused,

    #[error("Contract is in unpaused state")]
    ContractUnpaused,

    #[error("Try again later for nomination")]
    TryLaterForNomination,

    #[error("Contract method does not accept any funds")]
    NoFundsShouldBeAttached,

    #[error("Unauthorized function call")]
    Unauthorized,

    #[error("Invalid Pause Admin")]
    InvalidPauseAdmin,

    #[error("No active agents in active agent list")]
    NoActiveAgents,

    #[error("Invalid CronCat manager address")]
    InvalidCroncatManagerAddress { addr: String },

    #[error("Invalid CronCat tasks contract address")]
    InvalidTasksContractAddress { addr: String },

    #[error("Invalid version key, please update version key before calling external contracts")]
    InvalidVersionKey {},

    #[error("Unrecognised reply_id")]
    UnrecognisedReplyId { reply_id: u64 },

    #[error("An unexpected error occurred")]
    UnexpectedError {},

    #[error("Invalid callback data when deserializing data from execution result")]
    InvalidExecuteCallbackData {},

    #[error("No rewards available for withdraw")]
    NoWithdrawRewardsAvailable {},

    #[error("Invalid configuration value for: {field}")]
    InvalidConfigurationValue { field: String },
}
