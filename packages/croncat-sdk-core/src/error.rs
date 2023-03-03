use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SdkError {
    #[error("Invalid gas input")]
    InvalidGas {},
}
