use sylvia::cw_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Goal must be greater than zero")]
    ZeroGoal,

    #[error("Duplicate data entry")]
    DuplicateData,

    #[error("Data is not valid json")]
    InvalidJson,

    #[error("Unvalid donation")]
    NoDonation,

    #[error("Donation exceeds goal")]
    ExceedGoal,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Goal not reached yet")]
    GoalNotMet,

    #[error("Project is not validated")]
    NotValidated,

    #[error("Project is already validated")]
    AlreadyValidated,

    #[error("Project is already funded")]
    AlreadyFunded,

    #[error("Project is not yet ready for disbursal")]
    NotDisbursable,

    #[error("Project is already marked as disbursable")]
    AlreadyDisbursable,

    #[error("Project is already disbursed and completed")]
    AlreadyCompleted,

    #[error("Project is cancelled")]
    Cancelled,

    #[error("Project is already cancelled")]
    AlreadyCancelled,

    #[error("Project cannot be cancelled in its current status")]
    CannotCancel,

    #[error("No donation available to refund")]
    NoRefundAvailable,

    #[error("Project is not refundable")]
    NotRefundable,
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
