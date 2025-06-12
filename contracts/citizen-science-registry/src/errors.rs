use sylvia::cw_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Duplicate data entry")]
    DuplicateData,

    #[error("Data entry already verified")]
    AlreadyVerified,

    #[error("Data entry not verified yet")]
    NotVerified,

    #[error("Data entry already rewarded")]
    AlreadyRewarded,

    #[error("Data is not valid json")]
    InvalidJson,

    #[error("Not enought funds")]
    InvalidFunds,
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
