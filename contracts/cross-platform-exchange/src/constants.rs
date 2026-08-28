use crate::contract::{ExchangeRate, PartnerInfo};
use cw_storage_plus::{Item, Map};
use sylvia::cw_std::Addr;

pub const DEFAULT_DENOM: &str = "ustake";

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const DENOM: Item<String> = Item::new("denom");
pub const PARTNERS: Map<String, PartnerInfo> = Map::new("partners");
pub const RATES: Map<String, ExchangeRate> = Map::new("rates");
pub const LOCKED_BALANCES: Map<(Addr, String), sylvia::cw_std::Uint128> =
    Map::new("locked_balances");
