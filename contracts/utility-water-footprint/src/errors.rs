use sylvia::cw_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Company name is required")]
    EmptyName,

    #[error("Period is required")]
    EmptyPeriod,

    #[error("Usage amount must be greater than zero")]
    ZeroUsage,

    #[error("Savings cannot exceed usage")]
    IllogicalMetrics,

    #[error("Usage log is already validated")]
    AlreadyValidated,

    #[error("Usage log is not validated")]
    NotValidated,

    #[error("Certificate already issued for this company and period")]
    AlreadyIssued,

    #[error("Validated logs do not meet certificate criteria")]
    CriteriaNotMet,

    #[error("Verifier already exists")]
    VerifierAlreadyExists,

    #[error("Verifier not found")]
    VerifierNotFound,

    #[error("Data is not valid json")]
    InvalidJson,
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
