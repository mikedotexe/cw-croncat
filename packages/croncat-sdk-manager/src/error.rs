use cosmwasm_std::Uint128;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SdkError {
    #[error("Invalid gas input")]
    InvalidGas {},

    #[error("Not enough cw20 balance of {addr}, need {lack} more")]
    NotEnoughCw20 { addr: String, lack: Uint128 },

    #[error("Not enough native balance of {denom}, need {lack} more")]
    NotEnoughNative { denom: String, lack: Uint128 },

    #[error("Do not send extra coins, will be permanently lost")]
    NonRequiredDenom {},
}
