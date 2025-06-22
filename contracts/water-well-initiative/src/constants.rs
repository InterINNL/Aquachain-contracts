use crate::contract::Project;
use cw_storage_plus::{Item, Map};
use sylvia::cw_std::{Addr, Uint128};

pub const DEFAULT_DENOM: &str = "ustake";

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const PROJECTS: Map<u64, Project> = Map::new("projects");
pub const NEXT_PROJECT_ID: Item<u64> = Item::new("next_project_id");
pub const DATA_HASHES: Map<String, bool> = Map::new("data_hashes");
pub const DONATIONS: Map<(u64, &Addr), Uint128> = Map::new("donations");
pub const DENOM: Item<String> = Item::new("denom");
