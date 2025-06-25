use sylvia::cw_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Duplicate entry")]
    DuplicateData,

    #[error("Data entry already verified")]
    AlreadyVerified,

    #[error("Data entry not verified yet")]
    NotVerified,

    #[error("Data entry already rewarded")]
    AlreadyRewarded,

    #[error("Data is not valid json")]
    InvalidJson,

    #[error("Not enough funds")]
    InvalidFunds,

    #[error("Sensor is already activated")]
    AlreadyActivated,

    #[error("Sensor is already deactivated")]
    AlreadyDeactivated,

    #[error("Sensor is not active")]
    SensorInactive,

    #[error("Verifier already exists")]
    VerifierAlreadyExists,
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
