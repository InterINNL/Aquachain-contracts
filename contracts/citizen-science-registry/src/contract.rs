use crate::errors::ContractError;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::{Bound, Item, Map};
use hex;
use sha2::{Digest, Sha256};
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_std::{Addr, BankMsg, Coin, Order, Response, StdResult, Uint128};
use sylvia::entry_points;

#[cw_serde]
pub struct DataEntry {
    pub id: u64,
    pub submitter: Addr,
    pub data_str: String,
    pub verified: bool,
    pub verifier: Option<Addr>,
    pub rewarded: bool,
}

pub const OWNER: Item<Addr> = Item::new("owner");
pub const NEXT_ID: Item<u64> = Item::new("next_id");
pub const DATA_ENTRIES: Map<u64, DataEntry> = Map::new("data_entries");
pub const DATA_HASHES: Map<String, bool> = Map::new("data_hashes");
pub const VERIFIERS: Map<Addr, bool> = Map::new("verifiers");

pub const REWARD_AMOUNT: u128 = 1000000;
pub const ATOM: &str = "ustake";

pub struct CitizenScienceRegistry;

#[cfg_attr(not(feature = "library"), entry_points)]
#[contract]
#[sv::error(ContractError)]
impl CitizenScienceRegistry {
    pub const fn new() -> Self {
        Self
    }

    #[sv::msg(instantiate)]
    fn instantiate(&self, ctx: InstantiateCtx) -> StdResult<Response> {
        NEXT_ID.save(ctx.deps.storage, &1)?;
        OWNER.save(ctx.deps.storage, &ctx.info.sender)?;
        Ok(Response::new().add_attribute("method", "instantiate"))
    }

    /// Add a verifier address (only callable by contract owner)
    #[sv::msg(exec)]
    fn add_verifier(&self, ctx: ExecCtx, verifier: Addr) -> StdResult<Response> {
        let owner = OWNER.load(ctx.deps.storage)?;
        if ctx.info.sender != owner {
            return Err(ContractError::Unauthorized.into());
        }
        VERIFIERS.save(ctx.deps.storage, verifier.clone(), &true)?;
        Ok(Response::new()
            .add_attribute("action", "add_verifier")
            .add_attribute("verifier", verifier.to_string()))
    }

    #[sv::msg(exec)]
    fn submit_data(&self, ctx: ExecCtx, data: serde_json::Value) -> StdResult<Response> {
        // Reject empty JSON string, empty object, or empty array
        match &data {
            serde_json::Value::String(s) if s.is_empty() => {
                return Err(ContractError::InvalidJson.into());
            }
            serde_json::Value::Object(map) if map.is_empty() => {
                return Err(ContractError::InvalidJson.into());
            }
            serde_json::Value::Array(arr) if arr.is_empty() => {
                return Err(ContractError::InvalidJson.into());
            }
            _ => {}
        }

        // Serialize JSON value to string (to store/hash it)
        let data_str = serde_json::to_string(&data).map_err(|_| ContractError::InvalidJson)?;

        // Compute SHA-256 hash of the data string
        let mut hasher = Sha256::new();
        hasher.update(data_str.as_bytes());
        let data_hash_bytes = hasher.finalize();
        let data_hash = hex::encode(data_hash_bytes);

        // Check for duplicate data by looking up the hash
        if DATA_HASHES
            .may_load(ctx.deps.storage, data_hash.clone())?
            .unwrap_or(false)
        {
            return Err(ContractError::DuplicateData.into());
        }

        let id = NEXT_ID.load(ctx.deps.storage)?;

        let entry = DataEntry {
            id,
            submitter: ctx.info.sender.clone(),
            data_str: data_str.clone(),
            verified: false,
            verifier: None,
            rewarded: false,
        };

        DATA_ENTRIES.save(ctx.deps.storage, id, &entry)?;
        DATA_HASHES.save(ctx.deps.storage, data_hash.clone(), &true)?;
        NEXT_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "submit_data")
            .add_attribute("entry_id", id.to_string())
            .add_attribute("submitter", ctx.info.sender.to_string())
            .add_attribute("data_hash", data_hash))
    }

    #[sv::msg(exec)]
    fn verify_data(&self, ctx: ExecCtx, entry_id: u64) -> StdResult<Response> {
        if !VERIFIERS
            .may_load(ctx.deps.storage, ctx.info.sender.clone())?
            .unwrap_or(false)
        {
            return Err(ContractError::Unauthorized.into());
        }

        let mut entry = DATA_ENTRIES.load(ctx.deps.storage, entry_id)?;
        if entry.verified {
            return Err(ContractError::AlreadyVerified.into());
        }

        entry.verified = true;
        entry.verifier = Some(ctx.info.sender.clone());
        DATA_ENTRIES.save(ctx.deps.storage, entry_id, &entry)?;

        Ok(Response::new()
            .add_attribute("action", "verify_data")
            .add_attribute("entry_id", entry_id.to_string())
            .add_attribute("verifier", ctx.info.sender.to_string()))
    }

    #[sv::msg(exec)]
    fn reward_contributor(&self, ctx: ExecCtx, entry_id: u64) -> StdResult<Response> {
        let owner = OWNER.load(ctx.deps.storage)?;
        if ctx.info.sender != owner {
            return Err(ContractError::Unauthorized.into());
        }

        let mut entry = DATA_ENTRIES.load(ctx.deps.storage, entry_id)?;

        if !entry.verified {
            return Err(ContractError::NotVerified.into());
        }
        if entry.rewarded {
            return Err(ContractError::AlreadyRewarded.into());
        }

        // Check that caller provided exactly the right reward
        let sent = ctx.info.funds.iter().find(|c| c.denom == ATOM);
        match sent {
            Some(coin) if coin.amount == Uint128::from(REWARD_AMOUNT) => (),
            _ => return Err(ContractError::InvalidFunds.into()),
        }

        entry.rewarded = true;
        DATA_ENTRIES.save(ctx.deps.storage, entry_id, &entry)?;

        // Send the received funds to the contributor
        let send_msg = BankMsg::Send {
            to_address: entry.submitter.to_string(),
            amount: vec![Coin {
                denom: ATOM.to_string(),
                amount: Uint128::from(REWARD_AMOUNT),
            }],
        };

        Ok(Response::new()
            .add_message(send_msg)
            .add_attribute("action", "reward_contributor")
            .add_attribute("entry_id", entry_id.to_string())
            .add_attribute("recipient", entry.submitter.to_string())
            .add_attribute("amount", REWARD_AMOUNT.to_string()))
    }

    #[sv::msg(query)]
    fn get_data_entry(&self, ctx: QueryCtx, entry_id: u64) -> StdResult<DataEntry> {
        DATA_ENTRIES.load(ctx.deps.storage, entry_id)
    }

    #[sv::msg(query)]
    fn list_data_entries(
        &self,
        ctx: QueryCtx,
        start_after: Option<u64>,
        limit: Option<u32>,
    ) -> StdResult<Vec<DataEntry>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);

        DATA_ENTRIES
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| item.map(|(_, e)| e))
            .collect()
    }

    #[sv::msg(query)]
    fn list_verifiers(&self, ctx: QueryCtx) -> StdResult<Vec<Addr>> {
        let verifiers: StdResult<Vec<_>> = VERIFIERS
            .range(ctx.deps.storage, None, None, Order::Ascending)
            .map(|item| item.map(|(addr, _)| addr))
            .collect();
        verifiers
    }

    #[sv::msg(query)]
    fn is_verifier(&self, ctx: QueryCtx, verifier: Addr) -> StdResult<bool> {
        Ok(VERIFIERS
            .may_load(ctx.deps.storage, verifier)?
            .unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sylvia::cw_multi_test::IntoAddr;
    use sylvia::cw_std::CosmosMsg::Bank;
    use sylvia::cw_std::coin;
    use sylvia::cw_std::testing::{message_info, mock_dependencies, mock_env};

    #[test]
    fn init() {
        let owner = "owner".into_addr();
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(ctx).unwrap();

        let stored_owner = OWNER.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_owner, owner);

        let stored_next_id = NEXT_ID.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_next_id, 1);
    }

    #[test]
    fn add_verifier_unauthorized() {
        let owner = "owner".into_addr();
        let not_owner = "not_owner".into_addr();
        let verifier = "verifier".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        // Try to add verifier from a non-owner
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&not_owner, &[])));

        let err = contract.add_verifier(exec_ctx, verifier).unwrap_err();
        assert!(
            err.to_string().contains("Unauthorized"),
            "Expected unauthorized error, got: {err}"
        );
    }

    #[test]
    fn add_verifier_ok() {
        let owner = "owner".into_addr();
        let verifier = "verifier".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        // Add verifier as the owner
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let res = contract.add_verifier(exec_ctx, verifier.clone()).unwrap();

        // Check attributes
        let attrs = res.attributes;
        assert!(
            attrs
                .iter()
                .any(|attr| attr.key == "action" && attr.value == "add_verifier")
        );
        assert!(
            attrs
                .iter()
                .any(|attr| attr.key == "verifier" && attr.value == verifier.to_string())
        );

        let is_saved = VERIFIERS.load(deps.as_ref().storage, verifier).unwrap();
        assert!(is_saved);
    }

    #[test]
    fn submit_data_accepts_valid_json() {
        let sender = "user".into_addr();
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
        contract.instantiate(inst_ctx).unwrap();

        // Submit valid JSON
        let valid_json_str = r#"{"temperature": 24.5, "location": "field"}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
        let res = contract.submit_data(exec_ctx, valid_json);

        assert!(res.is_ok(), "Expected success on valid JSON");
    }

    #[test]
    fn submit_data_rejects_invalid_json_value() {
        let sender = "user".into_addr();
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
        contract.instantiate(inst_ctx).unwrap();

        let invalid_values = vec![
            serde_json::json!({}), // empty object
            serde_json::json!([]), // empty array
            serde_json::json!(""), // empty string
        ];

        for invalid_json in invalid_values {
            let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
            let err = contract.submit_data(exec_ctx, invalid_json).unwrap_err();

            assert!(
                err.to_string().contains("Data is not valid json"),
                "Expected InvalidJson error, got: {err}"
            );
        }
    }

    #[test]
    fn submit_data_duplicate_hash_rejected() {
        let user = "user".into_addr();
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
        contract.instantiate(inst_ctx).unwrap();

        let valid_json_str = r#"{"temp": 22}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
        contract.submit_data(exec_ctx, valid_json.clone()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
        let err = contract.submit_data(exec_ctx, valid_json).unwrap_err();
        assert!(err.to_string().contains("Duplicate"));
    }

    #[test]
    fn verify_data_success() {
        let owner = "owner".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        let valid_json_str = r#"{"data": "value"}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, valid_json).unwrap();

        // Verify data
        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        let res = contract.verify_data(verify_ctx, 1).unwrap();

        assert_eq!(
            res.attributes
                .iter()
                .find(|attr| attr.key == "action")
                .map(|attr| attr.value.as_str()),
            Some("verify_data")
        );
    }

    #[test]
    fn verify_data_unauthorized() {
        let owner = "owner".into_addr();
        let submitter = "submitter".into_addr();
        let not_verifier = "not_verifier".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        let valid_json_str = r#"{"some": "data"}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, valid_json).unwrap();

        let verify_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&not_verifier, &[])));
        let err = contract.verify_data(verify_ctx, 1).unwrap_err();

        assert!(
            err.to_string().contains("Unauthorized"),
            "Expected Unauthorized error, got: {err}"
        );
    }

    #[test]
    fn verify_data_already_verified() {
        let owner = "owner".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        // Submit data
        let valid_json_str = r#"{"x": 42}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, valid_json).unwrap();

        // First verification
        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        contract.verify_data(verify_ctx, 1).unwrap();

        // Second verification should fail
        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        let err = contract.verify_data(verify_ctx, 1).unwrap_err();

        assert!(
            err.to_string().contains("Data entry already verified"),
            "Expected AlreadyVerified error, got: {err}"
        );
    }

    #[test]
    fn reward_contributor_success() {
        let owner = "owner".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        let valid_json_str = r#"{"valid": true}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, valid_json).unwrap();

        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        contract.verify_data(verify_ctx, 1).unwrap();

        // Prepare reward funds
        let reward_funds = vec![coin(REWARD_AMOUNT, ATOM)];

        // Reward contributor with funds
        let reward_info = message_info(&owner, &reward_funds);
        let reward_ctx = ExecCtx::from((deps.as_mut(), mock_env(), reward_info));

        let res = contract.reward_contributor(reward_ctx, 1).unwrap();

        assert!(
            res.attributes
                .iter()
                .any(|attr| attr.key == "action" && attr.value == "reward_contributor"),
            "Missing reward_contributor action attribute"
        );
        assert!(
            res.messages
                .iter()
                .any(|msg| matches!(msg.msg, Bank(BankMsg::Send { .. }))),
            "Missing bank send message"
        );
    }

    #[test]
    fn reward_contributor_not_verified() {
        let owner = "owner".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        // Submit data
        let valid_json_str = r#"{"raw": "data"}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, valid_json).unwrap();

        // Try to reward without verification
        let reward_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let err = contract.reward_contributor(reward_ctx, 1).unwrap_err();

        assert!(
            err.to_string().contains("Data entry not verified yet"),
            "Expected NotVerified error, got: {err}"
        );
    }

    #[test]
    fn reward_contributor_already_rewarded() {
        let owner = "owner".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        // Add verifier
        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        // Submit and verify
        let valid_json_str = r#"{"hello": "world"}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, valid_json).unwrap();

        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        contract.verify_data(verify_ctx, 1).unwrap();

        // First reward call with enough funds (e.g. 1000 ATOM if that's the reward amount)
        let reward_funds = vec![coin(REWARD_AMOUNT, ATOM)];
        let reward_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&owner, &reward_funds),
        ));
        contract.reward_contributor(reward_ctx, 1).unwrap();

        // Second reward call with same sender (even with funds) should fail as already rewarded
        let reward_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&owner, &reward_funds),
        ));
        let err = contract.reward_contributor(reward_ctx, 1).unwrap_err();

        assert_eq!(err, ContractError::AlreadyRewarded.into());
    }

    #[test]
    fn get_data_entry_success() {
        let owner = "owner".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        let valid_json_str = r#"{"key": "value"}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract
            .submit_data(submit_ctx, valid_json.clone())
            .unwrap();

        // Query the data entry
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let entry = contract.get_data_entry(query_ctx, 1).unwrap();

        assert_eq!(entry.id, 1);
        assert_eq!(entry.data_str, valid_json.to_string());
        assert_eq!(entry.submitter, submitter);
        assert!(!entry.verified);
        assert!(entry.verifier.is_none());
        assert!(!entry.rewarded);
    }

    #[test]
    fn full_data_entry_lifecycle() {
        let owner = "owner".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        // Instantiate contract
        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        // Add verifier
        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        // Submit data by submitter
        let valid_json_str = r#"{"valid": true}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract
            .submit_data(submit_ctx, valid_json.clone())
            .unwrap();

        // Verify data by verifier
        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        contract.verify_data(verify_ctx, 1).unwrap();

        // Reward contributor: send funds along with the message
        let reward_funds = vec![coin(REWARD_AMOUNT, ATOM)];
        let reward_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&owner, &reward_funds),
        ));
        contract.reward_contributor(reward_ctx, 1).unwrap();

        // Query final entry and assert full state
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let entry = contract.get_data_entry(query_ctx, 1).unwrap();

        assert_eq!(entry.id, 1);
        assert_eq!(entry.data_str, valid_json.to_string());
        assert_eq!(entry.submitter, submitter);
        assert!(entry.verified);
        assert_eq!(entry.verifier, Some(verifier));
        assert!(entry.rewarded);
    }

    #[test]
    fn get_data_entry_not_found() {
        let owner = "owner".into_addr();
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        // Instantiate contract
        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        // Query for a non-existent entry
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let err = contract.get_data_entry(query_ctx, 42).unwrap_err();

        assert!(
            err.to_string().contains("not found"),
            "Expected not found error, got: {err}"
        );
    }

    #[test]
    fn list_data_entries_returns_paginated_entries() {
        let owner = "owner".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(ctx).unwrap();

        // Submit 3 data entries
        for i in 0..3 {
            let valid_json_str = format!(r#"{{"entry": {i}}}"#);
            let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
            let exec_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
            contract.submit_data(exec_ctx, valid_json).unwrap();
        }

        // Query entries
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let entries = contract
            .list_data_entries(query_ctx, None, Some(10))
            .unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].id, 1);
        assert_eq!(entries[2].id, 3);
    }

    #[test]
    fn list_verifiers_returns_all_registered_addresses() {
        let owner = "owner".into_addr();
        let verifier1 = "verifier1".into_addr();
        let verifier2 = "verifier2".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(ctx).unwrap();

        // Add verifiers
        let ctx1 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.add_verifier(ctx1, verifier1.clone()).unwrap();

        let ctx2 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.add_verifier(ctx2, verifier2.clone()).unwrap();

        // Query verifiers
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let verifiers = contract.list_verifiers(query_ctx).unwrap();

        assert_eq!(verifiers.len(), 2);
        assert!(verifiers.contains(&verifier1));
        assert!(verifiers.contains(&verifier2));
    }

    #[test]
    fn is_verifier_returns_true_for_registered_and_false_for_unregistered() {
        let owner = "owner".into_addr();
        let verifier1 = "verifier1".into_addr();
        let verifier2 = "verifier2".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(ctx).unwrap();

        // Add verifier1 as the owner
        let ctx1 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.add_verifier(ctx1, verifier1.clone()).unwrap();

        // Query verifier1 (should be true)
        let query_ctx1 = QueryCtx::from((deps.as_ref(), mock_env()));
        let is_verifier1 = contract.is_verifier(query_ctx1, verifier1.clone()).unwrap();
        assert!(is_verifier1);

        // Query verifier2 (not added, should be false)
        let query_ctx2 = QueryCtx::from((deps.as_ref(), mock_env()));
        let is_verifier2 = contract.is_verifier(query_ctx2, verifier2.clone()).unwrap();
        assert!(!is_verifier2);
    }
}
