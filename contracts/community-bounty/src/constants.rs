use crate::contract::{Bounty, WorkSubmission};
use cw_storage_plus::{Item, Map};
use sylvia::cw_std::Addr;

pub const DEFAULT_DENOM: &str = "ustake";

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const DENOM: Item<String> = Item::new("denom");
pub const NEXT_BOUNTY_ID: Item<u64> = Item::new("next_bounty_id");
pub const NEXT_SUBMISSION_ID: Item<u64> = Item::new("next_submission_id");
pub const BOUNTIES: Map<u64, Bounty> = Map::new("bounties");
pub const SUBMISSIONS: Map<u64, WorkSubmission> = Map::new("submissions");
