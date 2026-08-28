use sylvia::cw_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Invalid funds")]
    InvalidFunds,

    #[error("Invalid deadline")]
    InvalidDeadline,

    #[error("Title and location are required")]
    MissingFields,

    #[error("Bounty is not open")]
    BountyNotOpen,

    #[error("Submission deadline has passed")]
    DeadlinePassed,

    #[error("Submission not found")]
    SubmissionNotFound,

    #[error("Submission does not belong to bounty")]
    WrongBounty,

    #[error("Work payload is not valid json")]
    InvalidJson,

    #[error("Bounty cannot be cancelled")]
    CannotCancel,
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
