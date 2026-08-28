use sylvia::cw_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Duplicate evidence")]
    DuplicateEvidence,

    #[error("Evidence is not valid json")]
    InvalidJson,

    #[error("Impact points must be greater than zero")]
    ZeroImpact,

    #[error("Action is already verified")]
    AlreadyVerified,

    #[error("Action is not verified")]
    NotVerified,

    #[error("Action is already rewarded")]
    AlreadyRewarded,

    #[error("Reward funds required")]
    InvalidFunds,

    #[error("Verifier already exists")]
    VerifierAlreadyExists,

    #[error("Verifier not found")]
    VerifierNotFound,
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
