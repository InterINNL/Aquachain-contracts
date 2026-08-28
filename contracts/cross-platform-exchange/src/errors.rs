use sylvia::cw_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Partner denom is required")]
    MissingPartnerDenom,

    #[error("Partner already registered")]
    PartnerExists,

    #[error("Partner not found")]
    PartnerNotFound,

    #[error("Partner is inactive")]
    PartnerInactive,

    #[error("Rate not configured")]
    RateNotSet,

    #[error("Invalid rate")]
    InvalidRate,

    #[error("Amount must be greater than zero")]
    ZeroAmount,

    #[error("Amount does not divide evenly at the registered rate")]
    InexactAmount,

    #[error("Invalid funds")]
    InvalidFunds,

    #[error("Insufficient locked balance")]
    InsufficientLocked,

    #[error("Insufficient contract liquidity")]
    InsufficientLiquidity,
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
