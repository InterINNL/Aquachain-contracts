use crate::constants::{
    DEFAULT_QUORUM_BPS, DEFAULT_VOTING_PERIOD_SECONDS, NEXT_PROPOSAL_ID, PROPOSALS, QUORUM_BPS,
    TOTAL_VOTERS, VOTER_REGISTRY, VOTES, VOTING_PERIOD_SECONDS,
};
use crate::errors::ContractError;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::Bound;
use serde_json::{Value, to_string};
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_std::{Addr, Order, Response, StdResult};
use sylvia::entry_points;

#[cw_serde]
pub enum ProposalStatus {
    Open,
    Passed,
    Failed,
    Executed,
}

#[cw_serde]
pub enum VoteOption {
    Yes,
    No,
    Abstain,
}

#[cw_serde]
pub struct Proposal {
    pub id: u64,
    pub proposer: Addr,
    pub title: String,
    pub description: String,
    pub action_tag: String,
    pub metadata_str: String,
    pub status: ProposalStatus,
    pub yes_votes: u64,
    pub no_votes: u64,
    pub abstain_votes: u64,
    pub voting_end: u64,
    pub created_at: u64,
}

#[cw_serde]
pub struct VoteRecord {
    pub proposal_id: u64,
    pub voter: Addr,
    pub vote: VoteOption,
}

pub struct LocalDaoContract;

#[cfg_attr(not(feature = "library"), entry_points)]
#[contract]
#[sv::error(ContractError)]
impl LocalDaoContract {
    pub const fn new() -> Self {
        Self
    }

    #[sv::msg(instantiate)]
    fn instantiate(
        &self,
        ctx: InstantiateCtx,
        quorum_bps: Option<u64>,
        voting_period_seconds: Option<u64>,
    ) -> StdResult<Response> {
        let quorum = quorum_bps.unwrap_or(DEFAULT_QUORUM_BPS);
        let period = voting_period_seconds.unwrap_or(DEFAULT_VOTING_PERIOD_SECONDS);

        QUORUM_BPS.save(ctx.deps.storage, &quorum)?;
        VOTING_PERIOD_SECONDS.save(ctx.deps.storage, &period)?;
        NEXT_PROPOSAL_ID.save(ctx.deps.storage, &1)?;
        TOTAL_VOTERS.save(ctx.deps.storage, &0)?;

        Ok(Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("quorum_bps", quorum.to_string())
            .add_attribute("voting_period_seconds", period.to_string()))
    }

    #[sv::msg(exec)]
    fn create_proposal(
        &self,
        ctx: ExecCtx,
        title: String,
        description: String,
        action_tag: String,
        metadata: Value,
    ) -> StdResult<Response> {
        if title.trim().is_empty() {
            return Err(ContractError::MissingTitle.into());
        }
        if action_tag.trim().is_empty() {
            return Err(ContractError::MissingActionTag.into());
        }

        let metadata_str = parse_metadata(&metadata)?;
        let now = ctx.env.block.time.seconds();
        let period = VOTING_PERIOD_SECONDS.load(ctx.deps.storage)?;

        let id = NEXT_PROPOSAL_ID.load(ctx.deps.storage)?;
        let proposal = Proposal {
            id,
            proposer: ctx.info.sender.clone(),
            title: title.trim().to_string(),
            description: description.trim().to_string(),
            action_tag: action_tag.trim().to_string(),
            metadata_str,
            status: ProposalStatus::Open,
            yes_votes: 0,
            no_votes: 0,
            abstain_votes: 0,
            voting_end: now + period,
            created_at: now,
        };

        PROPOSALS.save(ctx.deps.storage, id, &proposal)?;
        NEXT_PROPOSAL_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "create_proposal")
            .add_attribute("proposal_id", id.to_string())
            .add_attribute("proposer", ctx.info.sender.to_string())
            .add_attribute("action_tag", proposal.action_tag.clone())
            .add_attribute("voting_end", proposal.voting_end.to_string()))
    }

    #[sv::msg(exec)]
    fn vote(&self, ctx: ExecCtx, proposal_id: u64, vote: VoteOption) -> StdResult<Response> {
        let mut proposal = PROPOSALS.load(ctx.deps.storage, proposal_id)?;
        if !matches!(proposal.status, ProposalStatus::Open) {
            return Err(ContractError::ProposalNotOpen.into());
        }
        let now = ctx.env.block.time.seconds();
        if now > proposal.voting_end {
            return Err(ContractError::VotingEnded.into());
        }

        let voter = ctx.info.sender.clone();
        if VOTES
            .may_load(ctx.deps.storage, (proposal_id, voter.clone()))?
            .is_some()
        {
            return Err(ContractError::AlreadyVoted.into());
        }

        register_voter(ctx.deps.storage, &voter)?;

        match vote {
            VoteOption::Yes => proposal.yes_votes += 1,
            VoteOption::No => proposal.no_votes += 1,
            VoteOption::Abstain => proposal.abstain_votes += 1,
        }

        let vote_label = format!("{vote:?}");
        PROPOSALS.save(ctx.deps.storage, proposal_id, &proposal)?;
        VOTES.save(
            ctx.deps.storage,
            (proposal_id, voter.clone()),
            &VoteRecord {
                proposal_id,
                voter: voter.clone(),
                vote,
            },
        )?;

        Ok(Response::new()
            .add_attribute("action", "vote")
            .add_attribute("proposal_id", proposal_id.to_string())
            .add_attribute("voter", voter.to_string())
            .add_attribute("vote", vote_label))
    }

    #[sv::msg(exec)]
    fn execute_proposal(&self, ctx: ExecCtx, proposal_id: u64) -> StdResult<Response> {
        let mut proposal = PROPOSALS.load(ctx.deps.storage, proposal_id)?;
        if !matches!(proposal.status, ProposalStatus::Open) {
            return Err(ContractError::AlreadyFinalized.into());
        }

        let now = ctx.env.block.time.seconds();
        if now <= proposal.voting_end {
            return Err(ContractError::VotingNotEnded.into());
        }

        let participation = proposal.yes_votes + proposal.no_votes + proposal.abstain_votes;
        let total_voters = TOTAL_VOTERS.load(ctx.deps.storage)?.max(1);
        let quorum_bps = QUORUM_BPS.load(ctx.deps.storage)?;
        let participation_bps = participation.saturating_mul(10_000) / total_voters;

        if participation_bps < quorum_bps {
            proposal.status = ProposalStatus::Failed;
            PROPOSALS.save(ctx.deps.storage, proposal_id, &proposal)?;
            return Ok(Response::new()
                .add_attribute("action", "execute_proposal")
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("result", "failed")
                .add_attribute("reason", "quorum_not_reached"));
        }

        if proposal.yes_votes <= proposal.no_votes {
            proposal.status = ProposalStatus::Failed;
            PROPOSALS.save(ctx.deps.storage, proposal_id, &proposal)?;
            return Ok(Response::new()
                .add_attribute("action", "execute_proposal")
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("result", "failed")
                .add_attribute("reason", "not_passed"));
        }

        proposal.status = ProposalStatus::Executed;
        PROPOSALS.save(ctx.deps.storage, proposal_id, &proposal)?;

        Ok(Response::new()
            .add_attribute("action", "execute_proposal")
            .add_attribute("proposal_id", proposal_id.to_string())
            .add_attribute("action_tag", proposal.action_tag.clone())
            .add_attribute("yes_votes", proposal.yes_votes.to_string())
            .add_attribute("no_votes", proposal.no_votes.to_string()))
    }

    #[sv::msg(query)]
    fn get_proposal(&self, ctx: QueryCtx, proposal_id: u64) -> StdResult<Proposal> {
        PROPOSALS.load(ctx.deps.storage, proposal_id)
    }

    #[sv::msg(query)]
    fn list_proposals(
        &self,
        ctx: QueryCtx,
        limit: Option<u32>,
        start_after: Option<u64>,
    ) -> StdResult<Vec<Proposal>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);

        PROPOSALS
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| item.map(|(_, p)| p))
            .collect()
    }

    #[sv::msg(query)]
    fn get_vote(&self, ctx: QueryCtx, proposal_id: u64, voter: Addr) -> StdResult<VoteRecord> {
        VOTES.load(ctx.deps.storage, (proposal_id, voter))
    }
}

fn parse_metadata(metadata: &Value) -> StdResult<String> {
    match metadata {
        Value::String(s) if s.trim().is_empty() => return Err(ContractError::InvalidJson.into()),
        Value::Object(map) if map.is_empty() => return Err(ContractError::InvalidJson.into()),
        Value::Array(arr) if arr.is_empty() => return Err(ContractError::InvalidJson.into()),
        _ => {}
    }

    to_string(metadata).map_err(|_| ContractError::InvalidJson.into())
}

fn register_voter(storage: &mut dyn sylvia::cw_std::Storage, voter: &Addr) -> StdResult<()> {
    if VOTER_REGISTRY
        .may_load(storage, voter.clone())?
        .unwrap_or(false)
    {
        return Ok(());
    }

    VOTER_REGISTRY.save(storage, voter.clone(), &true)?;
    let total = TOTAL_VOTERS.load(storage)?;
    TOTAL_VOTERS.save(storage, &(total + 1))?;
    Ok(())
}
