use crate::contract::{DataEntry, Sensor};
use cw_storage_plus::{Item, Map};
use sylvia::cw_std::Addr;

pub const DEFAULT_DENOM: &str = "ustake";

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const NEXT_SENSOR_ID: Item<u64> = Item::new("next_sensor_id");
pub const SENSORS: Map<u64, Sensor> = Map::new("sensors");
pub const SENSOR_HASHES: Map<String, bool> = Map::new("data_hashes");
pub const NEXT_ENTRY_ID: Item<u64> = Item::new("next_entry_id");
pub const DATA_ENTRIES: Map<u64, DataEntry> = Map::new("data_entries");
pub const DATA_HASHES: Map<(u64, String), bool> = Map::new("data_hashes");
pub const VERIFIERS: Map<Addr, bool> = Map::new("verifiers");
pub const NEXT_AGENT_ID: Item<u64> = Item::new("next_agent_id");
pub const AGENTS: Map<u64, crate::agent_registry::Agent> = Map::new("agents");
pub const AGENT_BY_OPERATOR: Map<Addr, u64> = Map::new("agent_by_operator");
pub const DENOM: Item<String> = Item::new("denom");
