use crate::contract::{Proposal, VoteRecord};
use cw_storage_plus::{Item, Map};
use sylvia::cw_std::Addr;

pub const DEFAULT_QUORUM_BPS: u64 = 3000;
pub const DEFAULT_VOTING_PERIOD_SECONDS: u64 = 604_800;
pub const DEFAULT_DENOM_VALUE: &str = "uosmo";

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const QUORUM_BPS: Item<u64> = Item::new("quorum_bps");
pub const DEFAULT_DENOM: Item<String> = Item::new("default_denom");
pub const COMMUNITY_BOUNTY: Item<Addr> = Item::new("community_bounty");
pub const WATER_CREDIT_MARKETPLACE: Item<Addr> = Item::new("water_credit_marketplace");
pub const CITIZEN_SCIENCE_REGISTRY: Item<Addr> = Item::new("citizen_science_registry");
pub const VOTING_PERIOD_SECONDS: Item<u64> = Item::new("voting_period_seconds");
pub const NEXT_PROPOSAL_ID: Item<u64> = Item::new("next_proposal_id");
pub const TOTAL_VOTERS: Item<u64> = Item::new("total_voters");
pub const PROPOSALS: Map<u64, Proposal> = Map::new("proposals");
pub const VOTES: Map<(u64, Addr), VoteRecord> = Map::new("votes");
pub const VOTER_REGISTRY: Map<Addr, bool> = Map::new("voter_registry");
