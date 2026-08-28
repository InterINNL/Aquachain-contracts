use crate::constants::{ADMIN, CREDIT_BALANCES, DEFAULT_DENOM, DENOM, LISTINGS, NEXT_LISTING_ID};
use crate::errors::ContractError;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::Bound;
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_std::{Addr, BankMsg, Coin, Order, Response, StdResult, Uint128};

#[cw_serde]
pub struct CreditListing {
    pub id: u64,
    pub seller: Addr,
    pub credits: Uint128,
    pub price: Uint128,
    pub region: String,
    pub expires_at: Option<u64>,
    pub active: bool,
    pub created_at: u64,
}

pub struct WaterCreditMarketplaceContract;

#[cfg_attr(not(feature = "library"), entry_points)]
#[contract]
#[sv::error(ContractError)]
impl WaterCreditMarketplaceContract {
    pub const fn new() -> Self {
        Self
    }

    #[sv::msg(instantiate)]
    fn instantiate(&self, ctx: InstantiateCtx, denom: Option<String>) -> StdResult<Response> {
        ADMIN.save(ctx.deps.storage, &ctx.info.sender)?;
        NEXT_LISTING_ID.save(ctx.deps.storage, &1)?;

        let denom_to_store = denom.unwrap_or_else(|| DEFAULT_DENOM.to_string());
        DENOM.save(ctx.deps.storage, &denom_to_store)?;

        Ok(Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("denom", denom_to_store))
    }

    #[sv::msg(exec)]
    fn mint_credits(&self, ctx: ExecCtx, recipient: Addr, amount: Uint128) -> StdResult<Response> {
        ensure_admin(&ctx)?;
        if amount.is_zero() {
            return Err(ContractError::ZeroCredits.into());
        }

        let prev = credit_balance(ctx.deps.storage, &recipient)?;
        CREDIT_BALANCES.save(ctx.deps.storage, recipient.clone(), &(prev + amount))?;

        Ok(Response::new()
            .add_attribute("action", "mint_credits")
            .add_attribute("recipient", recipient.to_string())
            .add_attribute("amount", amount.to_string()))
    }

    #[sv::msg(exec)]
    fn list_credit(
        &self,
        ctx: ExecCtx,
        credits: Uint128,
        price: Uint128,
        region: String,
        expires_at: Option<u64>,
    ) -> StdResult<Response> {
        if credits.is_zero() {
            return Err(ContractError::ZeroCredits.into());
        }
        if price.is_zero() {
            return Err(ContractError::ZeroPrice.into());
        }
        let now = ctx.env.block.time.seconds();
        if expires_at.is_some_and(|exp| exp <= now) {
            return Err(ContractError::InvalidExpiry.into());
        }

        let seller = ctx.info.sender.clone();
        let balance = credit_balance(ctx.deps.storage, &seller)?;
        if balance < credits {
            return Err(ContractError::InsufficientCredits.into());
        }

        CREDIT_BALANCES.save(ctx.deps.storage, seller.clone(), &(balance - credits))?;

        let id = NEXT_LISTING_ID.load(ctx.deps.storage)?;
        let listing = CreditListing {
            id,
            seller: seller.clone(),
            credits,
            price,
            region: region.trim().to_string(),
            expires_at,
            active: true,
            created_at: now,
        };

        LISTINGS.save(ctx.deps.storage, id, &listing)?;
        NEXT_LISTING_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "list_credit")
            .add_attribute("listing_id", id.to_string())
            .add_attribute("seller", seller.to_string())
            .add_attribute("credits", credits.to_string())
            .add_attribute("price", price.to_string()))
    }

    #[sv::msg(exec)]
    fn buy_credit(&self, ctx: ExecCtx, listing_id: u64) -> StdResult<Response> {
        let mut listing = LISTINGS.load(ctx.deps.storage, listing_id)?;
        if !listing.active {
            return Err(ContractError::ListingInactive.into());
        }
        let now = ctx.env.block.time.seconds();
        if listing.expires_at.is_some_and(|exp| now > exp) {
            return Err(ContractError::ListingExpired.into());
        }

        let stored_denom = DENOM.load(ctx.deps.storage)?;
        let payment = ctx
            .info
            .funds
            .iter()
            .find(|c| c.denom == stored_denom)
            .map(|c| c.amount)
            .ok_or(ContractError::InvalidFunds)?;
        if payment != listing.price {
            return Err(ContractError::WrongPrice.into());
        }

        listing.active = false;
        LISTINGS.save(ctx.deps.storage, listing_id, &listing)?;

        let buyer = ctx.info.sender.clone();
        let buyer_balance = credit_balance(ctx.deps.storage, &buyer)?;
        CREDIT_BALANCES.save(
            ctx.deps.storage,
            buyer.clone(),
            &(buyer_balance + listing.credits),
        )?;

        let payout = Coin {
            denom: stored_denom,
            amount: listing.price,
        };
        let send_msg = BankMsg::Send {
            to_address: listing.seller.to_string(),
            amount: vec![payout.clone()],
        };

        Ok(Response::new()
            .add_message(send_msg)
            .add_attribute("action", "buy_credit")
            .add_attribute("listing_id", listing_id.to_string())
            .add_attribute("buyer", buyer.to_string())
            .add_attribute("seller", listing.seller.to_string())
            .add_attribute("credits", listing.credits.to_string())
            .add_attribute("price", payout.amount.to_string()))
    }

    #[sv::msg(exec)]
    fn cancel_listing(&self, ctx: ExecCtx, listing_id: u64) -> StdResult<Response> {
        let mut listing = LISTINGS.load(ctx.deps.storage, listing_id)?;
        if ctx.info.sender != listing.seller {
            return Err(ContractError::Unauthorized.into());
        }
        if !listing.active {
            return Err(ContractError::ListingInactive.into());
        }

        listing.active = false;
        LISTINGS.save(ctx.deps.storage, listing_id, &listing)?;

        let seller_balance = credit_balance(ctx.deps.storage, &listing.seller)?;
        CREDIT_BALANCES.save(
            ctx.deps.storage,
            listing.seller.clone(),
            &(seller_balance + listing.credits),
        )?;

        Ok(Response::new()
            .add_attribute("action", "cancel_listing")
            .add_attribute("listing_id", listing_id.to_string())
            .add_attribute("seller", listing.seller.to_string())
            .add_attribute("credits_returned", listing.credits.to_string()))
    }

    #[sv::msg(exec)]
    fn transfer_credit(
        &self,
        ctx: ExecCtx,
        recipient: Addr,
        amount: Uint128,
    ) -> StdResult<Response> {
        if amount.is_zero() {
            return Err(ContractError::ZeroCredits.into());
        }

        let sender = ctx.info.sender.clone();
        let sender_balance = credit_balance(ctx.deps.storage, &sender)?;
        if sender_balance < amount {
            return Err(ContractError::InsufficientCredits.into());
        }

        CREDIT_BALANCES.save(ctx.deps.storage, sender.clone(), &(sender_balance - amount))?;

        let recipient_balance = credit_balance(ctx.deps.storage, &recipient)?;
        CREDIT_BALANCES.save(
            ctx.deps.storage,
            recipient.clone(),
            &(recipient_balance + amount),
        )?;

        Ok(Response::new()
            .add_attribute("action", "transfer_credit")
            .add_attribute("from", sender.to_string())
            .add_attribute("to", recipient.to_string())
            .add_attribute("amount", amount.to_string()))
    }

    #[sv::msg(query)]
    fn get_balance(&self, ctx: QueryCtx, address: Addr) -> StdResult<Uint128> {
        credit_balance(ctx.deps.storage, &address)
    }

    #[sv::msg(query)]
    fn get_listing(&self, ctx: QueryCtx, listing_id: u64) -> StdResult<CreditListing> {
        LISTINGS.load(ctx.deps.storage, listing_id)
    }

    #[sv::msg(query)]
    fn list_listings(
        &self,
        ctx: QueryCtx,
        active_only: Option<bool>,
        limit: Option<u32>,
        start_after: Option<u64>,
    ) -> StdResult<Vec<CreditListing>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);
        let active_only = active_only.unwrap_or(false);

        LISTINGS
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .filter_map(|item| match item {
                Ok((_, listing)) => {
                    if active_only && !listing.active {
                        None
                    } else {
                        Some(Ok(listing))
                    }
                }
                Err(e) => Some(Err(e)),
            })
            .take(limit)
            .collect()
    }
}

fn credit_balance(storage: &dyn sylvia::cw_std::Storage, address: &Addr) -> StdResult<Uint128> {
    Ok(CREDIT_BALANCES
        .may_load(storage, address.clone())?
        .unwrap_or_else(Uint128::zero))
}

fn ensure_admin(ctx: &ExecCtx) -> StdResult<()> {
    let admin = ADMIN.load(ctx.deps.storage)?;
    if ctx.info.sender != admin {
        return Err(ContractError::Unauthorized.into());
    }
    Ok(())
}
