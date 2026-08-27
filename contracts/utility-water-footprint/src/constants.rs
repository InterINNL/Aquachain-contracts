use crate::contract::{Certificate, Company, UsageLog};
use cw_storage_plus::{Item, Map};
use sylvia::cw_std::Addr;

pub const DEFAULT_DENOM: &str = "ustake";

/// Minimum net savings ratio in basis points (1000 = 10%) required to issue a certificate.
pub const MIN_SAVINGS_RATIO_BPS: u128 = 1000;

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const DENOM: Item<String> = Item::new("denom");

pub const NEXT_COMPANY_ID: Item<u64> = Item::new("next_company_id");
pub const COMPANIES: Map<u64, Company> = Map::new("companies");

pub const NEXT_LOG_ID: Item<u64> = Item::new("next_log_id");
pub const USAGE_LOGS: Map<u64, UsageLog> = Map::new("usage_logs");

pub const NEXT_CERT_ID: Item<u64> = Item::new("next_cert_id");
pub const CERTIFICATES: Map<u64, Certificate> = Map::new("certificates");
pub const CERT_BY_COMPANY_PERIOD: Map<(u64, String), u64> = Map::new("cert_by_company_period");

pub const VERIFIERS: Map<Addr, bool> = Map::new("verifiers");
