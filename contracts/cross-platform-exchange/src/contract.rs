use crate::constants::{ADMIN, DEFAULT_DENOM, DENOM, LOCKED_BALANCES, PARTNERS, RATES};
use crate::errors::ContractError;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::Bound;
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_std::{Addr, BankMsg, Coin, Order, Response, StdResult, Uint128};
use sylvia::entry_points;

#[cw_serde]
pub struct PartnerInfo {
    pub denom: String,
    pub label: String,
    pub region: String,
    pub active: bool,
}

#[cw_serde]
pub struct ExchangeRate {
    pub partner_denom: String,
    pub base_amount: Uint128,
    pub partner_amount: Uint128,
}

#[cw_serde]
pub enum SwapDirection {
    BaseToPartner,
    PartnerToBase,
}

pub struct CrossPlatformExchangeContract;

#[cfg_attr(not(feature = "library"), entry_points)]
#[contract]
#[sv::error(ContractError)]
impl CrossPlatformExchangeContract {
    pub const fn new() -> Self {
        Self
    }

    #[sv::msg(instantiate)]
    fn instantiate(&self, ctx: InstantiateCtx, denom: Option<String>) -> StdResult<Response> {
        ADMIN.save(ctx.deps.storage, &ctx.info.sender)?;

        let denom_to_store = denom.unwrap_or_else(|| DEFAULT_DENOM.to_string());
        DENOM.save(ctx.deps.storage, &denom_to_store)?;

        Ok(Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("denom", denom_to_store))
    }

    #[sv::msg(exec)]
    fn register_partner(
        &self,
        ctx: ExecCtx,
        denom: String,
        label: String,
        region: String,
    ) -> StdResult<Response> {
        ensure_admin(&ctx)?;
        let key = normalize_partner_denom(&denom)?;
        if PARTNERS.may_load(ctx.deps.storage, key.clone())?.is_some() {
            return Err(ContractError::PartnerExists.into());
        }

        let partner = PartnerInfo {
            denom: key.clone(),
            label: label.trim().to_string(),
            region: region.trim().to_string(),
            active: true,
        };
        PARTNERS.save(ctx.deps.storage, key.clone(), &partner)?;

        Ok(Response::new()
            .add_attribute("action", "register_partner")
            .add_attribute("partner_denom", key))
    }

    #[sv::msg(exec)]
    fn set_rate(
        &self,
        ctx: ExecCtx,
        partner_denom: String,
        base_amount: Uint128,
        partner_amount: Uint128,
    ) -> StdResult<Response> {
        ensure_admin(&ctx)?;
        let key = normalize_partner_denom(&partner_denom)?;
        let partner = load_active_partner(ctx.deps.storage, &key)?;

        if base_amount.is_zero() || partner_amount.is_zero() {
            return Err(ContractError::InvalidRate.into());
        }

        let rate = ExchangeRate {
            partner_denom: partner.denom.clone(),
            base_amount,
            partner_amount,
        };
        RATES.save(ctx.deps.storage, key.clone(), &rate)?;

        Ok(Response::new()
            .add_attribute("action", "set_rate")
            .add_attribute("partner_denom", key)
            .add_attribute("base_amount", base_amount.to_string())
            .add_attribute("partner_amount", partner_amount.to_string()))
    }

    #[sv::msg(exec)]
    fn swap(
        &self,
        ctx: ExecCtx,
        partner_denom: String,
        direction: SwapDirection,
        amount: Uint128,
    ) -> StdResult<Response> {
        if amount.is_zero() {
            return Err(ContractError::ZeroAmount.into());
        }

        let key = normalize_partner_denom(&partner_denom)?;
        let _partner = load_active_partner(ctx.deps.storage, &key)?;
        let rate = RATES
            .may_load(ctx.deps.storage, key.clone())?
            .ok_or(ContractError::RateNotSet)?;

        let base_denom = DENOM.load(ctx.deps.storage)?;
        let trader = ctx.info.sender.clone();

        match direction {
            SwapDirection::BaseToPartner => {
                let payment = ctx
                    .info
                    .funds
                    .iter()
                    .find(|c| c.denom == base_denom)
                    .map(|c| c.amount)
                    .ok_or(ContractError::InvalidFunds)?;
                if payment != amount {
                    return Err(ContractError::InvalidFunds.into());
                }

                let partner_out = partner_from_base(amount, &rate)?;
                let prev = locked_balance(ctx.deps.storage, &trader, &key)?;
                LOCKED_BALANCES.save(
                    ctx.deps.storage,
                    (trader.clone(), key.clone()),
                    &(prev + partner_out),
                )?;

                Ok(Response::new()
                    .add_attribute("action", "swap")
                    .add_attribute("direction", "base_to_partner")
                    .add_attribute("partner_denom", key)
                    .add_attribute("trader", trader.to_string())
                    .add_attribute("base_in", amount.to_string())
                    .add_attribute("partner_out", partner_out.to_string()))
            }
            SwapDirection::PartnerToBase => {
                let locked = locked_balance(ctx.deps.storage, &trader, &key)?;
                if locked < amount {
                    return Err(ContractError::InsufficientLocked.into());
                }

                let base_out = base_from_partner(amount, &rate)?;
                let contract_balance = ctx
                    .deps
                    .querier
                    .query_balance(ctx.env.contract.address.clone(), base_denom.clone())?
                    .amount;
                if contract_balance < base_out {
                    return Err(ContractError::InsufficientLiquidity.into());
                }

                LOCKED_BALANCES.save(
                    ctx.deps.storage,
                    (trader.clone(), key.clone()),
                    &(locked - amount),
                )?;

                let payout = Coin {
                    denom: base_denom,
                    amount: base_out,
                };

                Ok(Response::new()
                    .add_message(BankMsg::Send {
                        to_address: trader.to_string(),
                        amount: vec![payout.clone()],
                    })
                    .add_attribute("action", "swap")
                    .add_attribute("direction", "partner_to_base")
                    .add_attribute("partner_denom", key)
                    .add_attribute("trader", trader.to_string())
                    .add_attribute("partner_in", amount.to_string())
                    .add_attribute("base_out", payout.amount.to_string()))
            }
        }
    }

    #[sv::msg(exec)]
    fn withdraw(
        &self,
        ctx: ExecCtx,
        partner_denom: String,
        amount: Uint128,
    ) -> StdResult<Response> {
        if amount.is_zero() {
            return Err(ContractError::ZeroAmount.into());
        }

        let key = normalize_partner_denom(&partner_denom)?;
        let _partner = load_active_partner(ctx.deps.storage, &key)?;
        let owner = ctx.info.sender.clone();
        let locked = locked_balance(ctx.deps.storage, &owner, &key)?;
        if locked < amount {
            return Err(ContractError::InsufficientLocked.into());
        }

        LOCKED_BALANCES.save(
            ctx.deps.storage,
            (owner.clone(), key.clone()),
            &(locked - amount),
        )?;

        Ok(Response::new()
            .add_attribute("action", "withdraw")
            .add_attribute("partner_denom", key)
            .add_attribute("owner", owner.to_string())
            .add_attribute("amount", amount.to_string()))
    }

    #[sv::msg(query)]
    fn get_partner(&self, ctx: QueryCtx, partner_denom: String) -> StdResult<PartnerInfo> {
        let key = normalize_partner_denom(&partner_denom)?;
        PARTNERS
            .load(ctx.deps.storage, key)
            .map_err(|_| ContractError::PartnerNotFound.into())
    }

    #[sv::msg(query)]
    fn list_partners(
        &self,
        ctx: QueryCtx,
        active_only: Option<bool>,
        limit: Option<u32>,
        start_after: Option<String>,
    ) -> StdResult<Vec<PartnerInfo>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = match start_after {
            Some(s) => Some(Bound::exclusive(normalize_partner_denom(&s)?)),
            None => None,
        };
        let active_only = active_only.unwrap_or(false);

        PARTNERS
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .filter_map(|item| match item {
                Ok((_, partner)) => {
                    if active_only && !partner.active {
                        None
                    } else {
                        Some(Ok(partner))
                    }
                }
                Err(e) => Some(Err(e)),
            })
            .take(limit)
            .collect()
    }

    #[sv::msg(query)]
    fn get_rate(&self, ctx: QueryCtx, partner_denom: String) -> StdResult<ExchangeRate> {
        let key = normalize_partner_denom(&partner_denom)?;
        RATES
            .load(ctx.deps.storage, key)
            .map_err(|_| ContractError::RateNotSet.into())
    }

    #[sv::msg(query)]
    fn get_locked_balance(
        &self,
        ctx: QueryCtx,
        address: Addr,
        partner_denom: String,
    ) -> StdResult<Uint128> {
        let key = normalize_partner_denom(&partner_denom)?;
        locked_balance(ctx.deps.storage, &address, &key)
    }
}

fn normalize_partner_denom(denom: &str) -> StdResult<String> {
    let trimmed = denom.trim();
    if trimmed.is_empty() {
        return Err(ContractError::MissingPartnerDenom.into());
    }
    Ok(trimmed.to_string())
}

fn load_active_partner(
    storage: &dyn sylvia::cw_std::Storage,
    key: &str,
) -> StdResult<PartnerInfo> {
    let partner = PARTNERS
        .load(storage, key.to_string())
        .map_err(|_| ContractError::PartnerNotFound)?;
    if !partner.active {
        return Err(ContractError::PartnerInactive.into());
    }
    Ok(partner)
}

fn locked_balance(
    storage: &dyn sylvia::cw_std::Storage,
    address: &Addr,
    partner_denom: &str,
) -> StdResult<Uint128> {
    Ok(LOCKED_BALANCES
        .may_load(storage, (address.clone(), partner_denom.to_string()))?
        .unwrap_or_else(Uint128::zero))
}

fn partner_from_base(amount: Uint128, rate: &ExchangeRate) -> StdResult<Uint128> {
    let product = amount
        .checked_mul(rate.partner_amount)
        .map_err(|_| ContractError::InexactAmount)?;
    let remainder = product
        .checked_rem(rate.base_amount)
        .map_err(|_| ContractError::InexactAmount)?;
    if !remainder.is_zero() {
        return Err(ContractError::InexactAmount.into());
    }
    Ok(product / rate.base_amount)
}

fn base_from_partner(amount: Uint128, rate: &ExchangeRate) -> StdResult<Uint128> {
    let product = amount
        .checked_mul(rate.base_amount)
        .map_err(|_| ContractError::InexactAmount)?;
    let remainder = product
        .checked_rem(rate.partner_amount)
        .map_err(|_| ContractError::InexactAmount)?;
    if !remainder.is_zero() {
        return Err(ContractError::InexactAmount.into());
    }
    Ok(product / rate.partner_amount)
}

fn ensure_admin(ctx: &ExecCtx) -> StdResult<()> {
    let admin = ADMIN.load(ctx.deps.storage)?;
    if ctx.info.sender != admin {
        return Err(ContractError::Unauthorized.into());
    }
    Ok(())
}
