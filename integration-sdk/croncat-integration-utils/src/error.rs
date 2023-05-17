use cosmwasm_std::{Addr, StdError};
use cw_utils::ParseReplyError;
use serde_json::Error as SerdeJsonError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum CronCatContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Parse Reply Error {0}")]
    ParseReplyError(#[from] ParseReplyError),

    #[error("Reply {reply_id} failed")]
    ReplyError { reply_id: u64 },

    #[error("Failed to retrieve latest task execution info from potential CronCat manager: {manager_addr}")]
    LatestTaskInfoFailed { manager_addr: Addr },

    #[error("Could not deserialize task info")]
    DeserializeTaskInfo {},

    #[error("No response from factory regarding potential manager ({manager_addr}) for version {version}")]
    FactoryManagerQueryFailed { manager_addr: Addr, version: String },

    #[error("Attempted invocation from unsanctioned manager contract ({manager_addr}) for version {version}")]
    UnsanctionedInvocation { manager_addr: Addr, version: String },

    #[error("Invocation not in the same block and transaction index")]
    NotSameBlockTxIndex {},

    #[error("Invocation not called by task owner. Expected owner: {expected_owner}")]
    WrongTaskOwner { expected_owner: Addr },

    #[error("Serialization error: {msg}")]
    SerdeError { msg: String },

    #[error("Must attach funds for task creation")]
    TaskCreationNoFunds,

    #[error("No contract named {contract_name} on factory {factory_addr}")]
    NoSuchContractOnFactory {
        contract_name: String,
        factory_addr: Addr,
    },
}

impl From<SerdeJsonError> for CronCatContractError {
    fn from(error: SerdeJsonError) -> Self {
        CronCatContractError::SerdeError {
            msg: format!("Serialization error: {}", error),
        }
    }
}
