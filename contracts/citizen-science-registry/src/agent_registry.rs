use cosmwasm_schema::cw_serde;
use sylvia::cw_std::Addr;

#[cw_serde]
pub enum AgentType {
    Drone,
    Verifier,
}

#[cw_serde]
pub struct Agent {
    pub id: u64,
    pub name: String,
    pub agent_type: AgentType,
    pub operator: Addr,
    pub pubkey: String,
    pub policy_json: String,
    pub registered_at: u64,
}

pub fn validate_agent_name(name: &str) -> bool {
    !name.trim().is_empty()
}

pub fn validate_pubkey(pubkey: &str) -> bool {
    !pubkey.trim().is_empty()
}

pub fn serialize_policy(policy: &serde_json::Value) -> Result<String, ()> {
    serde_json::to_string(policy).map_err(|_| ())
}
