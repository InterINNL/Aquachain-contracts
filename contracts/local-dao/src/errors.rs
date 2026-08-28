use sylvia::cw_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Title is required")]
    MissingTitle,

    #[error("Action tag is required")]
    MissingActionTag,

    #[error("Metadata is not valid json")]
    InvalidJson,

    #[error("Proposal is not open")]
    ProposalNotOpen,

    #[error("Voting period has not ended")]
    VotingNotEnded,

    #[error("Voting period has ended")]
    VotingEnded,

    #[error("Already voted")]
    AlreadyVoted,

    #[error("Quorum not reached")]
    QuorumNotReached,

    #[error("Proposal did not pass")]
    ProposalNotPassed,

    #[error("Proposal already finalized")]
    AlreadyFinalized,
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
