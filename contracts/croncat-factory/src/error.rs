use cosmwasm_std::StdError;
use cw_utils::ParseReplyError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error(transparent)]
    ParseReplyError(#[from] ParseReplyError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Unknown contract name")]
    UnknownContract {},

    #[error("Unknown contract method")]
    UnknownMethod {},

    #[error("Can't remove latest version")]
    LatestVersionRemove {},

    #[error("Can't deploy this version, it already exists")]
    VersionExists {},

    #[error("Can't remove contract unless it's paused or library")]
    NotPaused {},

    #[error("Provided URL has exceeded the maximum allowable length")]
    UrlExceededMaxLength {},

    #[error("Must not nominate current owner")]
    SameOwnerNominated {},
}
