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

    #[error("Insufficient credit balance")]
    InsufficientCredits,

    #[error("Listing is not active")]
    ListingInactive,

    #[error("Listing has expired")]
    ListingExpired,

    #[error("Credits must be greater than zero")]
    ZeroCredits,

    #[error("Price must be greater than zero")]
    ZeroPrice,

    #[error("Invalid expiry")]
    InvalidExpiry,

    #[error("Payment must match listing price")]
    WrongPrice,
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
