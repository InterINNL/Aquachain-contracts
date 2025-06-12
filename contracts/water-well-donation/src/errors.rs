use sylvia::cw_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Goal must be greater than zero")]
    ZeroGoal,

    #[error("No valid donation")]
    NoDonation,

    #[error("Donation exceeds goal")]
    ExceedGoal,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Goal not reached yet")]
    GoalNotMet,

    #[error("Already disbursed")]
    AlreadyDisbursed,
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
