use crate::constants::{
    ACTIONS, ACTOR_IMPACT, ADMIN, DEFAULT_DENOM, DENOM, EVIDENCE_HASHES, NEXT_ACTION_ID, VERIFIERS,
};
use crate::errors::ContractError;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::Bound;
use hex;
use serde_json::{Value, to_string};
use sha2::{Digest, Sha256};
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_std::{Addr, BankMsg, Coin, Order, Response, StdResult, Uint128};
use sylvia::entry_points;

#[cw_serde]
pub struct EcoAction {
    pub id: u64,
    pub actor: Addr,
    pub evidence_str: String,
    pub impact_points: Uint128,
    pub verified: bool,
    pub rewarded: bool,
    pub validator: Option<Addr>,
    pub created_at: u64,
}

pub struct SustainableActionRewardsContract;

#[cfg_attr(not(feature = "library"), entry_points)]
#[contract]
#[sv::error(ContractError)]
impl SustainableActionRewardsContract {
    pub const fn new() -> Self {
        Self
    }

    #[sv::msg(instantiate)]
    fn instantiate(&self, ctx: InstantiateCtx, denom: Option<String>) -> StdResult<Response> {
        ADMIN.save(ctx.deps.storage, &ctx.info.sender)?;
        NEXT_ACTION_ID.save(ctx.deps.storage, &1)?;

        let denom_to_store = denom.unwrap_or_else(|| DEFAULT_DENOM.to_string());
        DENOM.save(ctx.deps.storage, &denom_to_store)?;

        Ok(Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("denom", denom_to_store))
    }

    #[sv::msg(exec)]
    fn submit_action(&self, ctx: ExecCtx, evidence: Value) -> StdResult<Response> {
        let (evidence_str, impact_points) = parse_evidence(&evidence)?;
        let evidence_hash = hash_evidence(&evidence_str)?;

        if EVIDENCE_HASHES
            .may_load(ctx.deps.storage, evidence_hash.clone())?
            .unwrap_or(false)
        {
            return Err(ContractError::DuplicateEvidence.into());
        }

        let id = NEXT_ACTION_ID.load(ctx.deps.storage)?;
        let action = EcoAction {
            id,
            actor: ctx.info.sender.clone(),
            evidence_str: evidence_str.clone(),
            impact_points,
            verified: false,
            rewarded: false,
            validator: None,
            created_at: ctx.env.block.time.seconds(),
        };

        ACTIONS.save(ctx.deps.storage, id, &action)?;
        EVIDENCE_HASHES.save(ctx.deps.storage, evidence_hash.clone(), &true)?;
        NEXT_ACTION_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "submit_action")
            .add_attribute("action_id", id.to_string())
            .add_attribute("actor", ctx.info.sender.to_string())
            .add_attribute("evidence_hash", evidence_hash))
    }

    #[sv::msg(exec)]
    fn verify_action(&self, ctx: ExecCtx, action_id: u64) -> StdResult<Response> {
        ensure_admin_or_verifier(&ctx)?;

        let mut eco = ACTIONS.load(ctx.deps.storage, action_id)?;
        if eco.verified {
            return Err(ContractError::AlreadyVerified.into());
        }

        eco.verified = true;
        eco.validator = Some(ctx.info.sender.clone());
        ACTIONS.save(ctx.deps.storage, action_id, &eco)?;

        let prev = ACTOR_IMPACT
            .may_load(ctx.deps.storage, eco.actor.clone())?
            .unwrap_or(0);
        ACTOR_IMPACT.save(
            ctx.deps.storage,
            eco.actor.clone(),
            &(prev + eco.impact_points.u128()),
        )?;

        Ok(Response::new()
            .add_attribute("action", "verify_action")
            .add_attribute("action_id", action_id.to_string())
            .add_attribute("validator", ctx.info.sender.to_string())
            .add_attribute("actor", eco.actor.to_string())
            .add_attribute("impact_points", eco.impact_points.to_string()))
    }

    #[sv::msg(exec)]
    fn reward_actor(&self, ctx: ExecCtx, action_id: u64) -> StdResult<Response> {
        ensure_admin(&ctx)?;

        let mut eco = ACTIONS.load(ctx.deps.storage, action_id)?;
        if !eco.verified {
            return Err(ContractError::NotVerified.into());
        }
        if eco.rewarded {
            return Err(ContractError::AlreadyRewarded.into());
        }

        let stored_denom = DENOM.load(ctx.deps.storage)?;
        let reward_amount = ctx
            .info
            .funds
            .iter()
            .find(|c| c.denom == stored_denom)
            .map(|c| c.amount)
            .ok_or(ContractError::InvalidFunds)?;

        eco.rewarded = true;
        ACTIONS.save(ctx.deps.storage, action_id, &eco)?;

        let send_msg = BankMsg::Send {
            to_address: eco.actor.to_string(),
            amount: vec![Coin {
                denom: stored_denom,
                amount: reward_amount,
            }],
        };

        Ok(Response::new()
            .add_message(send_msg)
            .add_attribute("action", "reward_actor")
            .add_attribute("action_id", action_id.to_string())
            .add_attribute("recipient", eco.actor.to_string())
            .add_attribute("amount", reward_amount.to_string()))
    }

    #[sv::msg(exec)]
    fn add_verifier(&self, ctx: ExecCtx, verifier: Addr) -> StdResult<Response> {
        ensure_admin(&ctx)?;

        if VERIFIERS
            .may_load(ctx.deps.storage, verifier.clone())?
            .unwrap_or(false)
        {
            return Err(ContractError::VerifierAlreadyExists.into());
        }

        VERIFIERS.save(ctx.deps.storage, verifier.clone(), &true)?;
        Ok(Response::new()
            .add_attribute("action", "add_verifier")
            .add_attribute("verifier", verifier.to_string()))
    }

    #[sv::msg(exec)]
    fn remove_verifier(&self, ctx: ExecCtx, verifier: Addr) -> StdResult<Response> {
        ensure_admin(&ctx)?;

        if !VERIFIERS
            .may_load(ctx.deps.storage, verifier.clone())?
            .unwrap_or(false)
        {
            return Err(ContractError::VerifierNotFound.into());
        }

        VERIFIERS.remove(ctx.deps.storage, verifier.clone());
        Ok(Response::new()
            .add_attribute("action", "remove_verifier")
            .add_attribute("verifier", verifier.to_string()))
    }

    #[sv::msg(query)]
    fn get_action(&self, ctx: QueryCtx, action_id: u64) -> StdResult<EcoAction> {
        ACTIONS.load(ctx.deps.storage, action_id)
    }

    #[sv::msg(query)]
    fn list_actions(
        &self,
        ctx: QueryCtx,
        limit: Option<u32>,
        start_after: Option<u64>,
    ) -> StdResult<Vec<EcoAction>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);

        ACTIONS
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| item.map(|(_, a)| a))
            .collect()
    }

    #[sv::msg(query)]
    fn get_actor_impact(&self, ctx: QueryCtx, actor: Addr) -> StdResult<Uint128> {
        Ok(Uint128::from(
            ACTOR_IMPACT.may_load(ctx.deps.storage, actor)?.unwrap_or(0),
        ))
    }

    #[sv::msg(query)]
    fn is_verifier(&self, ctx: QueryCtx, address: Addr) -> StdResult<bool> {
        Ok(VERIFIERS
            .may_load(ctx.deps.storage, address)?
            .unwrap_or(false))
    }
}

fn parse_evidence(evidence: &Value) -> StdResult<(String, Uint128)> {
    match evidence {
        Value::String(s) if s.is_empty() => return Err(ContractError::InvalidJson.into()),
        Value::Object(map) if map.is_empty() => return Err(ContractError::InvalidJson.into()),
        Value::Array(arr) if arr.is_empty() => return Err(ContractError::InvalidJson.into()),
        _ => {}
    }

    let evidence_str = to_string(evidence).map_err(|_| ContractError::InvalidJson)?;
    let impact_raw = evidence
        .get("impact_points")
        .ok_or(ContractError::InvalidJson)?;

    let impact_str = match impact_raw {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return Err(ContractError::InvalidJson.into()),
    };

    let impact_points = impact_str
        .trim()
        .parse::<u128>()
        .map_err(|_| ContractError::InvalidJson)?;
    let impact_points = Uint128::from(impact_points);
    if impact_points.is_zero() {
        return Err(ContractError::ZeroImpact.into());
    }

    Ok((evidence_str, impact_points))
}

fn hash_evidence(evidence_str: &str) -> StdResult<String> {
    let mut hasher = Sha256::new();
    hasher.update(evidence_str.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

fn ensure_admin(ctx: &ExecCtx) -> StdResult<()> {
    let admin = ADMIN.load(ctx.deps.storage)?;
    if ctx.info.sender != admin {
        return Err(ContractError::Unauthorized.into());
    }
    Ok(())
}

fn ensure_admin_or_verifier(ctx: &ExecCtx) -> StdResult<()> {
    let admin = ADMIN.load(ctx.deps.storage)?;
    if ctx.info.sender == admin {
        return Ok(());
    }
    if VERIFIERS
        .may_load(ctx.deps.storage, ctx.info.sender.clone())?
        .unwrap_or(false)
    {
        return Ok(());
    }
    Err(ContractError::Unauthorized.into())
}
