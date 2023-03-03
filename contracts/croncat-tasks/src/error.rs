use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Boundary is not in valid format")]
    InvalidBoundary {},

    #[error("Invalid interval")]
    InvalidInterval {},

    #[error("Empty balance, must attach funds")]
    MustAttach {},

    #[error("invalid cosmwasm message")]
    InvalidWasmMsg {},

    #[error("Actions message unsupported or invalid message data")]
    InvalidAction {},

    #[error("Supplied address is not valid address")]
    InvalidAddress {},

    #[error("Invalid gas input")]
    InvalidGas {},

    #[error("Must provide gas limit for WASM actions")]
    NoGasLimit {},

    #[error("Contract is in paused state")]
    ContractPaused,

    #[error("Contract is in unpaused state")]
    ContractUnpaused,

    #[error("Task ended")]
    TaskEnded {},

    #[error("Task already exists")]
    TaskExists {},

    #[error("No task found by hash")]
    NoTaskFound {},

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid Pause Admin")]
    InvalidPauseAdmin,

    #[error("Chain name can't be longer than 32 characters")]
    TooLongChainName {},

    #[error("Invalid version key, please update it")]
    InvalidKey {},

    #[error("Field must be non-zero: {field}")]
    InvalidZeroValue { field: String },
}
