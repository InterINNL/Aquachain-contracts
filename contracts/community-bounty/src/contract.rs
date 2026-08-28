use crate::constants::{
    ADMIN, BOUNTIES, DEFAULT_DENOM, DENOM, NEXT_BOUNTY_ID, NEXT_SUBMISSION_ID, SUBMISSIONS,
};
use crate::errors::ContractError;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::Bound;
use serde_json::{Value, to_string};
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_std::{Addr, BankMsg, Coin, Order, Response, StdResult, Uint128};

#[cw_serde]
pub enum BountyStatus {
    Open,
    Completed,
    Cancelled,
}

#[cw_serde]
pub struct Bounty {
    pub id: u64,
    pub poster: Addr,
    pub title: String,
    pub description: String,
    pub location: String,
    pub deadline: u64,
    pub reward_amount: Uint128,
    pub status: BountyStatus,
    pub winner: Option<Addr>,
    pub approved_submission_id: Option<u64>,
    pub created_at: u64,
}

#[cw_serde]
pub struct WorkSubmission {
    pub id: u64,
    pub bounty_id: u64,
    pub worker: Addr,
    pub work_str: String,
    pub submitted_at: u64,
    pub approved: bool,
}

pub struct CommunityBountyContract;

#[cfg_attr(not(feature = "library"), entry_points)]
#[contract]
#[sv::error(ContractError)]
impl CommunityBountyContract {
    pub const fn new() -> Self {
        Self
    }

    #[sv::msg(instantiate)]
    fn instantiate(&self, ctx: InstantiateCtx, denom: Option<String>) -> StdResult<Response> {
        ADMIN.save(ctx.deps.storage, &ctx.info.sender)?;
        NEXT_BOUNTY_ID.save(ctx.deps.storage, &1)?;
        NEXT_SUBMISSION_ID.save(ctx.deps.storage, &1)?;

        let denom_to_store = denom.unwrap_or_else(|| DEFAULT_DENOM.to_string());
        DENOM.save(ctx.deps.storage, &denom_to_store)?;

        Ok(Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("denom", denom_to_store))
    }

    #[sv::msg(exec)]
    fn post_bounty(
        &self,
        ctx: ExecCtx,
        title: String,
        description: String,
        location: String,
        deadline: u64,
    ) -> StdResult<Response> {
        if title.trim().is_empty() || location.trim().is_empty() {
            return Err(ContractError::MissingFields.into());
        }
        let now = ctx.env.block.time.seconds();
        if deadline <= now {
            return Err(ContractError::InvalidDeadline.into());
        }

        let stored_denom = DENOM.load(ctx.deps.storage)?;
        let reward_amount = ctx
            .info
            .funds
            .iter()
            .find(|c| c.denom == stored_denom)
            .map(|c| c.amount)
            .ok_or(ContractError::InvalidFunds)?;
        if reward_amount.is_zero() {
            return Err(ContractError::InvalidFunds.into());
        }

        let id = NEXT_BOUNTY_ID.load(ctx.deps.storage)?;
        let bounty = Bounty {
            id,
            poster: ctx.info.sender.clone(),
            title: title.trim().to_string(),
            description: description.trim().to_string(),
            location: location.trim().to_string(),
            deadline,
            reward_amount,
            status: BountyStatus::Open,
            winner: None,
            approved_submission_id: None,
            created_at: now,
        };

        BOUNTIES.save(ctx.deps.storage, id, &bounty)?;
        NEXT_BOUNTY_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "post_bounty")
            .add_attribute("bounty_id", id.to_string())
            .add_attribute("poster", ctx.info.sender.to_string())
            .add_attribute("reward_amount", reward_amount.to_string())
            .add_attribute("deadline", deadline.to_string()))
    }

    #[sv::msg(exec)]
    fn submit_work(&self, ctx: ExecCtx, bounty_id: u64, work: Value) -> StdResult<Response> {
        let bounty = BOUNTIES.load(ctx.deps.storage, bounty_id)?;
        if !matches!(bounty.status, BountyStatus::Open) {
            return Err(ContractError::BountyNotOpen.into());
        }
        let now = ctx.env.block.time.seconds();
        if now > bounty.deadline {
            return Err(ContractError::DeadlinePassed.into());
        }

        let work_str = parse_work(&work)?;

        let id = NEXT_SUBMISSION_ID.load(ctx.deps.storage)?;
        let submission = WorkSubmission {
            id,
            bounty_id,
            worker: ctx.info.sender.clone(),
            work_str,
            submitted_at: now,
            approved: false,
        };

        SUBMISSIONS.save(ctx.deps.storage, id, &submission)?;
        NEXT_SUBMISSION_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "submit_work")
            .add_attribute("bounty_id", bounty_id.to_string())
            .add_attribute("submission_id", id.to_string())
            .add_attribute("worker", ctx.info.sender.to_string()))
    }

    #[sv::msg(exec)]
    fn approve_work(
        &self,
        ctx: ExecCtx,
        bounty_id: u64,
        submission_id: u64,
    ) -> StdResult<Response> {
        let mut bounty = BOUNTIES.load(ctx.deps.storage, bounty_id)?;
        if ctx.info.sender != bounty.poster {
            return Err(ContractError::Unauthorized.into());
        }
        if !matches!(bounty.status, BountyStatus::Open) {
            return Err(ContractError::BountyNotOpen.into());
        }

        let mut submission = SUBMISSIONS.load(ctx.deps.storage, submission_id)?;
        if submission.bounty_id != bounty_id {
            return Err(ContractError::WrongBounty.into());
        }

        let stored_denom = DENOM.load(ctx.deps.storage)?;
        let payout = Coin {
            denom: stored_denom.clone(),
            amount: bounty.reward_amount,
        };

        submission.approved = true;
        SUBMISSIONS.save(ctx.deps.storage, submission_id, &submission)?;

        bounty.status = BountyStatus::Completed;
        bounty.winner = Some(submission.worker.clone());
        bounty.approved_submission_id = Some(submission_id);
        BOUNTIES.save(ctx.deps.storage, bounty_id, &bounty)?;

        let send_msg = BankMsg::Send {
            to_address: submission.worker.to_string(),
            amount: vec![payout.clone()],
        };

        Ok(Response::new()
            .add_message(send_msg)
            .add_attribute("action", "approve_work")
            .add_attribute("bounty_id", bounty_id.to_string())
            .add_attribute("submission_id", submission_id.to_string())
            .add_attribute("winner", submission.worker.to_string())
            .add_attribute("amount", payout.amount.to_string()))
    }

    #[sv::msg(exec)]
    fn cancel_bounty(&self, ctx: ExecCtx, bounty_id: u64) -> StdResult<Response> {
        let mut bounty = BOUNTIES.load(ctx.deps.storage, bounty_id)?;
        if ctx.info.sender != bounty.poster {
            return Err(ContractError::Unauthorized.into());
        }
        if !matches!(bounty.status, BountyStatus::Open) {
            return Err(ContractError::CannotCancel.into());
        }

        let stored_denom = DENOM.load(ctx.deps.storage)?;
        let refund = Coin {
            denom: stored_denom,
            amount: bounty.reward_amount,
        };

        bounty.status = BountyStatus::Cancelled;
        BOUNTIES.save(ctx.deps.storage, bounty_id, &bounty)?;

        let send_msg = BankMsg::Send {
            to_address: bounty.poster.to_string(),
            amount: vec![refund.clone()],
        };

        Ok(Response::new()
            .add_message(send_msg)
            .add_attribute("action", "cancel_bounty")
            .add_attribute("bounty_id", bounty_id.to_string())
            .add_attribute("poster", bounty.poster.to_string())
            .add_attribute("refund_amount", refund.amount.to_string()))
    }

    #[sv::msg(query)]
    fn get_bounty(&self, ctx: QueryCtx, bounty_id: u64) -> StdResult<Bounty> {
        BOUNTIES.load(ctx.deps.storage, bounty_id)
    }

    #[sv::msg(query)]
    fn list_bounties(
        &self,
        ctx: QueryCtx,
        limit: Option<u32>,
        start_after: Option<u64>,
    ) -> StdResult<Vec<Bounty>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);

        BOUNTIES
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| item.map(|(_, b)| b))
            .collect()
    }

    #[sv::msg(query)]
    fn list_submissions(
        &self,
        ctx: QueryCtx,
        bounty_id: Option<u64>,
        limit: Option<u32>,
        start_after: Option<u64>,
    ) -> StdResult<Vec<WorkSubmission>> {
        let limit = limit.unwrap_or(20).min(50) as usize;
        let start = start_after.map(Bound::exclusive);

        SUBMISSIONS
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .filter_map(|item| match item {
                Ok((_, sub)) => {
                    if bounty_id.is_some_and(|id| id != sub.bounty_id) {
                        None
                    } else {
                        Some(Ok(sub))
                    }
                }
                Err(e) => Some(Err(e)),
            })
            .take(limit)
            .collect()
    }
}

fn parse_work(work: &Value) -> StdResult<String> {
    match work {
        Value::String(s) if s.trim().is_empty() => return Err(ContractError::InvalidJson.into()),
        Value::Object(map) if map.is_empty() => return Err(ContractError::InvalidJson.into()),
        Value::Array(arr) if arr.is_empty() => return Err(ContractError::InvalidJson.into()),
        _ => {}
    }

    to_string(work).map_err(|_| ContractError::InvalidJson.into())
}
