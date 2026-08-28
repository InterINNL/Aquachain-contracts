use crate::contract::CreditListing;
use cw_storage_plus::{Item, Map};
use sylvia::cw_std::{Addr, Uint128};

pub const DEFAULT_DENOM: &str = "ustake";

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const DENOM: Item<String> = Item::new("denom");
pub const NEXT_LISTING_ID: Item<u64> = Item::new("next_listing_id");
pub const CREDIT_BALANCES: Map<Addr, Uint128> = Map::new("credit_balances");
pub const LISTINGS: Map<u64, CreditListing> = Map::new("listings");
