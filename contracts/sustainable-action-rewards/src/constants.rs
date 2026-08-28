use crate::contract::EcoAction;
use cw_storage_plus::{Item, Map};
use sylvia::cw_std::Addr;

pub const DEFAULT_DENOM: &str = "ustake";

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const DENOM: Item<String> = Item::new("denom");

pub const NEXT_ACTION_ID: Item<u64> = Item::new("next_action_id");
pub const ACTIONS: Map<u64, EcoAction> = Map::new("actions");
pub const EVIDENCE_HASHES: Map<String, bool> = Map::new("evidence_hashes");
pub const ACTOR_IMPACT: Map<Addr, u128> = Map::new("actor_impact");

pub const VERIFIERS: Map<Addr, bool> = Map::new("verifiers");
