use crate::constants::{
    ADMIN, DATA_ENTRIES, DATA_HASHES, DEFAULT_DENOM, DENOM, NEXT_ENTRY_ID, NEXT_SENSOR_ID,
    SENSOR_HASHES, SENSORS, VERIFIERS,
};
use crate::enums::SensorStatus;
use crate::errors::ContractError;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::Bound;
use hex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_std::{Addr, BankMsg, Coin, Order, Response, StdResult};
use sylvia::entry_points;

#[cw_serde]
pub struct Sensor {
    pub id: u64,
    pub owner: Addr,
    pub data_str: String,
    pub status: SensorStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

#[cw_serde]
pub struct DataEntry {
    pub id: u64,
    pub sensor_id: u64,
    pub submitter: Addr,
    pub data_str: String,
    pub verified: bool,
    pub verifier: Option<Addr>,
    pub rewarded: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct CitizenScienceRegistry;

#[cfg_attr(not(feature = "library"), entry_points)]
#[contract]
#[sv::error(ContractError)]
impl CitizenScienceRegistry {
    pub const fn new() -> Self {
        Self
    }

    #[sv::msg(instantiate)]
    fn instantiate(&self, ctx: InstantiateCtx, denom: Option<String>) -> StdResult<Response> {
        ADMIN.save(ctx.deps.storage, &ctx.info.sender)?;
        NEXT_SENSOR_ID.save(ctx.deps.storage, &1)?;
        NEXT_ENTRY_ID.save(ctx.deps.storage, &1)?;

        let denom_to_store = denom.unwrap_or_else(|| DEFAULT_DENOM.to_string());
        DENOM.save(ctx.deps.storage, &denom_to_store)?;

        Ok(Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("denom", denom_to_store))
    }

    #[sv::msg(exec)]
    fn submit_sensor(&self, ctx: ExecCtx, data: Value) -> StdResult<Response> {
        // Reject empty values
        match &data {
            Value::String(s) if s.is_empty() => {
                return Err(ContractError::InvalidJson.into());
            }
            Value::Object(map) if map.is_empty() => {
                return Err(ContractError::InvalidJson.into());
            }
            Value::Array(arr) if arr.is_empty() => {
                return Err(ContractError::InvalidJson.into());
            }
            _ => {}
        }

        // Serialize JSON value to string
        let data_str = serde_json::to_string(&data).map_err(|_| ContractError::InvalidJson)?;

        // Compute SHA-256 hash of the data string
        let mut hasher = Sha256::new();
        hasher.update(data_str.as_bytes());
        let data_hash_bytes = hasher.finalize();
        let data_hash = hex::encode(data_hash_bytes);

        // Check if sensor with the same hash already exists
        if SENSOR_HASHES
            .may_load(ctx.deps.storage, data_hash.clone())?
            .unwrap_or(false)
        {
            return Err(ContractError::DuplicateData.into());
        }

        let id = NEXT_SENSOR_ID.load(ctx.deps.storage)?;
        let block_time = ctx.env.block.time.seconds();

        let sensor = Sensor {
            id,
            owner: ctx.info.sender.clone(),
            data_str: data_str.clone(),
            status: SensorStatus::Proposed,
            created_at: block_time,
            updated_at: block_time,
        };

        SENSORS.save(ctx.deps.storage, id, &sensor)?;
        SENSOR_HASHES.save(ctx.deps.storage, data_hash.clone(), &true)?;
        NEXT_SENSOR_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "submit_sensor")
            .add_attribute("sensor_id", id.to_string())
            .add_attribute("owner", ctx.info.sender.to_string())
            .add_attribute("sensor_hash", data_hash))
    }

    #[sv::msg(exec)]
    fn activate(&self, ctx: ExecCtx, sensor_id: u64) -> StdResult<Response> {
        let admin = ADMIN.load(ctx.deps.storage)?;
        if ctx.info.sender != admin {
            return Err(ContractError::Unauthorized.into());
        }

        let mut sensor = SENSORS.load(ctx.deps.storage, sensor_id)?;
        if matches!(sensor.status, SensorStatus::Active) {
            return Err(ContractError::AlreadyActivated.into());
        }

        sensor.status = SensorStatus::Active;
        sensor.updated_at = ctx.env.block.time.seconds();
        SENSORS.save(ctx.deps.storage, sensor_id, &sensor)?;

        Ok(Response::new()
            .add_attribute("action", "activate")
            .add_attribute("sensor_id", sensor_id.to_string()))
    }

    #[sv::msg(exec)]
    fn deactivate(&self, ctx: ExecCtx, sensor_id: u64) -> StdResult<Response> {
        let admin = ADMIN.load(ctx.deps.storage)?;
        if ctx.info.sender != admin {
            return Err(ContractError::Unauthorized.into());
        }

        let mut sensor = SENSORS.load(ctx.deps.storage, sensor_id)?;
        if matches!(sensor.status, SensorStatus::Inactive) {
            return Err(ContractError::AlreadyDeactivated.into());
        }

        sensor.status = SensorStatus::Inactive;
        sensor.updated_at = ctx.env.block.time.seconds();
        SENSORS.save(ctx.deps.storage, sensor_id, &sensor)?;

        Ok(Response::new()
            .add_attribute("action", "deactivate")
            .add_attribute("sensor_id", sensor_id.to_string()))
    }

    #[sv::msg(exec)]
    fn delete(&self, ctx: ExecCtx, sensor_id: u64) -> StdResult<Response> {
        let admin = ADMIN.load(ctx.deps.storage)?;
        if ctx.info.sender != admin {
            return Err(ContractError::Unauthorized.into());
        }

        SENSORS.load(ctx.deps.storage, sensor_id)?;

        SENSORS.remove(ctx.deps.storage, sensor_id);

        Ok(Response::new()
            .add_attribute("action", "delete")
            .add_attribute("sensor_id", sensor_id.to_string()))
    }

    /// Add a verifier address (only callable by contract admin)
    #[sv::msg(exec)]
    fn add_verifier(&self, ctx: ExecCtx, verifier: Addr) -> StdResult<Response> {
        let admin = ADMIN.load(ctx.deps.storage)?;
        let sender = ctx.info.sender;
        if sender != admin {
            return Err(ContractError::Unauthorized.into());
        }

        if VERIFIERS
            .may_load(ctx.deps.storage, verifier.clone())?
            .unwrap_or(false)
        {
            return Err(ContractError::VerifierAlreadyExists.into());
        }

        VERIFIERS.save(ctx.deps.storage, verifier.clone(), &true)?;
        Ok(Response::new()
            .add_attribute("action", "add_verifier")
            .add_attribute("verifier", verifier.to_string()))
    }

    #[sv::msg(exec)]
    fn submit_data(&self, ctx: ExecCtx, sensor_id: u64, data: Value) -> StdResult<Response> {
        let sensor = SENSORS.load(ctx.deps.storage, sensor_id)?;

        if !matches!(sensor.status, SensorStatus::Active) {
            return Err(ContractError::SensorInactive.into());
        }

        // Reject empty JSON string, empty object, or empty array
        match &data {
            Value::String(s) if s.is_empty() => {
                return Err(ContractError::InvalidJson.into());
            }
            Value::Object(map) if map.is_empty() => {
                return Err(ContractError::InvalidJson.into());
            }
            Value::Array(arr) if arr.is_empty() => {
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
        let hash_key = (sensor_id, data_hash.clone());

        // Check for duplicate data by looking up the hash
        if DATA_HASHES
            .may_load(ctx.deps.storage, hash_key.clone())?
            .unwrap_or(false)
        {
            return Err(ContractError::DuplicateData.into());
        }

        let id = NEXT_ENTRY_ID.load(ctx.deps.storage)?;
        let block_time = ctx.env.block.time.seconds();

        let entry = DataEntry {
            id,
            sensor_id,
            submitter: ctx.info.sender.clone(),
            data_str: data_str.clone(),
            verified: false,
            verifier: None,
            rewarded: false,
            created_at: block_time,
            updated_at: block_time,
        };

        DATA_ENTRIES.save(ctx.deps.storage, id, &entry)?;
        DATA_HASHES.save(ctx.deps.storage, hash_key, &true)?;
        NEXT_ENTRY_ID.save(ctx.deps.storage, &(id + 1))?;

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
        entry.updated_at = ctx.env.block.time.seconds(); // Update timestamp here

        DATA_ENTRIES.save(ctx.deps.storage, entry_id, &entry)?;

        Ok(Response::new()
            .add_attribute("action", "verify_data")
            .add_attribute("entry_id", entry_id.to_string())
            .add_attribute("submitter", entry.submitter.to_string())
            .add_attribute("verifier", ctx.info.sender.clone()))
    }

    #[sv::msg(exec)]
    fn reward_submitter(&self, ctx: ExecCtx, entry_id: u64) -> StdResult<Response> {
        let admin = ADMIN.load(ctx.deps.storage)?;
        let sender = ctx.info.sender;
        if sender != admin {
            return Err(ContractError::Unauthorized.into());
        }

        let mut entry = DATA_ENTRIES.load(ctx.deps.storage, entry_id)?;

        if !entry.verified {
            return Err(ContractError::NotVerified.into());
        }
        if entry.rewarded {
            return Err(ContractError::AlreadyRewarded.into());
        }

        let stored_denom = DENOM.load(ctx.deps.storage)?;

        // Find the sent coin with stored denom
        let sent_coin = ctx.info.funds.iter().find(|c| c.denom == stored_denom);
        let reward_amount = match sent_coin {
            Some(coin) => coin.amount,
            None => return Err(ContractError::InvalidFunds.into()),
        };

        // Mark as rewarded
        entry.rewarded = true;
        DATA_ENTRIES.save(ctx.deps.storage, entry_id, &entry)?;

        // Send the received funds to the submitter
        let send_msg = BankMsg::Send {
            to_address: entry.submitter.to_string(),
            amount: vec![Coin {
                denom: stored_denom,
                amount: reward_amount,
            }],
        };

        Ok(Response::new()
            .add_message(send_msg)
            .add_attribute("action", "reward_submitter")
            .add_attribute("entry_id", entry_id.to_string())
            .add_attribute("sender", sender.to_string())
            .add_attribute("recipient", entry.submitter.to_string())
            .add_attribute("amount", reward_amount.to_string()))
    }

    #[sv::msg(query)]
    fn get_sensor(&self, ctx: QueryCtx, sensor_id: u64) -> StdResult<Sensor> {
        SENSORS.load(ctx.deps.storage, sensor_id)
    }

    #[sv::msg(query)]
    fn get_data_entry(&self, ctx: QueryCtx, entry_id: u64) -> StdResult<DataEntry> {
        DATA_ENTRIES.load(ctx.deps.storage, entry_id)
    }

    #[sv::msg(query)]
    fn count_sensors(
        &self,
        ctx: QueryCtx,
        owner: Option<Addr>,
        status: Option<SensorStatus>,
    ) -> StdResult<u64> {
        let count = SENSORS
            .range(ctx.deps.storage, None, None, Order::Descending)
            .filter_map(|item| match item {
                Ok((_, sensor)) => {
                    if let Some(o) = &owner
                        && sensor.owner != *o
                    {
                        return None;
                    }

                    if let Some(s) = &status
                        && &sensor.status != s
                    {
                        return None;
                    }

                    Some(())
                }
                Err(_) => None,
            })
            .count() as u64;

        Ok(count)
    }

    #[sv::msg(query)]
    fn list_sensors(
        &self,
        ctx: QueryCtx,
        start_after: Option<u64>,
        limit: Option<u32>,
        owner: Option<Addr>,
        status: Option<SensorStatus>,
    ) -> StdResult<Vec<Sensor>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);

        let sensors: Vec<Sensor> = SENSORS
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .filter_map(|item| match item {
                Ok((_, sensor)) => {
                    if let Some(o) = &owner
                        && sensor.owner != o
                    {
                        return None;
                    }

                    if let Some(s) = &status
                        && &sensor.status != s
                    {
                        return None;
                    }

                    Some(Ok(sensor))
                }
                Err(e) => Some(Err(e)),
            })
            .collect::<StdResult<Vec<Sensor>>>()?;

        Ok(sensors.into_iter().take(limit).collect())
    }

    #[sv::msg(query)]
    fn count_data_entries(
        &self,
        ctx: QueryCtx,
        submitter: Option<Addr>,
        sensor_id: Option<u64>,
    ) -> StdResult<u64> {
        let count = DATA_ENTRIES
            .range(ctx.deps.storage, None, None, Order::Descending)
            .filter_map(|item| match item {
                Ok((_, entry)) => {
                    if let Some(s) = &submitter
                        && entry.submitter != s
                    {
                        return None;
                    }
                    if let Some(id) = sensor_id
                        && entry.sensor_id != id
                    {
                        return None;
                    }
                    Some(()) // Count this entry
                }
                Err(_) => None,
            })
            .count() as u64;

        Ok(count)
    }

    #[sv::msg(query)]
    fn list_data_entries(
        &self,
        ctx: QueryCtx,
        start_after: Option<u64>,
        limit: Option<u32>,
        submitter: Option<Addr>,
        sensor_id: Option<u64>,
    ) -> StdResult<Vec<DataEntry>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);

        let entries: Vec<DataEntry> = DATA_ENTRIES
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .filter_map(|item| match item {
                Ok((_, entry)) => {
                    // Filter by submitter if specified
                    if let Some(submitter) = &submitter
                        && entry.submitter != submitter
                    {
                        return None;
                    }

                    // Filter by sensor_id if specified
                    if let Some(id) = sensor_id
                        && entry.sensor_id != id
                    {
                        return None;
                    }

                    Some(Ok(entry))
                }
                Err(e) => Some(Err(e)),
            })
            .collect::<StdResult<Vec<DataEntry>>>()?;

        Ok(entries.into_iter().take(limit).collect())
    }

    #[sv::msg(query)]
    fn list_verifiers(&self, ctx: QueryCtx) -> StdResult<Vec<Addr>> {
        let verifiers: StdResult<Vec<_>> = VERIFIERS
            .range(ctx.deps.storage, None, None, Order::Descending)
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
pub mod tests {
    use super::*;
    use sylvia::cw_multi_test::IntoAddr;
    use sylvia::cw_std::CosmosMsg::Bank;
    use sylvia::cw_std::coin;
    use sylvia::cw_std::testing::{message_info, mock_dependencies, mock_env};

    pub const REWARD_AMOUNT: u128 = 10;

    #[test]
    fn instantiate() {
        let admin = "admin".into_addr();
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let stored_admin = ADMIN.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_admin, admin);

        let stored_next_sensor_id = NEXT_SENSOR_ID.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_next_sensor_id, 1);

        let stored_next_entry_id = NEXT_ENTRY_ID.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_next_entry_id, 1);
    }

    #[test]
    fn instantiate_defaults_to_ustake_if_denom_not_provided() {
        let admin = "admin".into_addr();
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        // Call instantiate without providing a denom
        contract.instantiate(ctx, None).unwrap();

        // Load and assert the default denom is used
        let stored_denom = DENOM.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_denom, "ustake".to_string());
    }

    #[test]
    fn submit_sensor_rejects_invalid_json_values() {
        let sender = Addr::unchecked("user");
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let invalid_inputs = vec![
            serde_json::json!(""), // Empty string
            serde_json::json!([]), // Empty array
            serde_json::json!({}), // Empty object
        ];

        for invalid_json in invalid_inputs {
            let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
            let err = contract.submit_sensor(exec_ctx, invalid_json).unwrap_err();
            assert!(
                err.to_string().contains("Data is not valid json"),
                "Expected InvalidJson error, got: {err}"
            );
        }
    }

    #[test]
    fn submit_sensor_accepts_valid_json() {
        let submitter = "submitter".into_addr();
        let admin = Addr::unchecked("admin");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json_str = r#"{"name": "field sensor", "location": "field"}"#;
        let valid_json: serde_json::Value = serde_json::from_str(valid_json_str).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));

        // Call submit_sensor with valid JSON
        let res = contract.submit_sensor(exec_ctx, valid_json);

        assert!(
            res.is_ok(),
            "Expected submit_sensor to succeed with valid JSON"
        );

        // Verify sensor stored correctly
        let sensor = SENSORS
            .load(deps.as_ref().storage, 1)
            .expect("Sensor should exist");

        assert_eq!(sensor.id, 1);
        assert_eq!(sensor.owner, submitter);
        assert!(sensor.data_str.contains("field sensor"));
        assert_eq!(sensor.status, SensorStatus::Proposed);
    }

    #[test]
    fn submit_sensor_increments_id_for_multiple_sensors() {
        let sender = Addr::unchecked("user");
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json1 = serde_json::json!({"name": "sensor1"});
        let valid_json2 = serde_json::json!({"name": "sensor2"});

        // Submit first sensor
        let exec_ctx1 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
        let res1 = contract.submit_sensor(exec_ctx1, valid_json1);
        assert!(res1.is_ok());

        // Submit second sensor
        let exec_ctx2 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
        let res2 = contract.submit_sensor(exec_ctx2, valid_json2);
        assert!(res2.is_ok());

        // Check sensors stored with incremented ids
        let sensor1 = SENSORS
            .load(deps.as_ref().storage, 1)
            .expect("Sensor 1 missing");
        let sensor2 = SENSORS
            .load(deps.as_ref().storage, 2)
            .expect("Sensor 2 missing");

        assert_eq!(sensor1.id, 1);
        assert_eq!(sensor1.owner, sender);
        assert!(sensor1.data_str.contains("sensor1"));

        assert_eq!(sensor2.id, 2);
        assert_eq!(sensor2.owner, sender);
        assert!(sensor2.data_str.contains("sensor2"));
    }

    #[test]
    fn submit_sensor_records_correct_owner() {
        let user1 = Addr::unchecked("user1");
        let user2 = Addr::unchecked("user2");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&user1, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let sensor_json = serde_json::json!({"name": "owner 1 test"});

        // user1 submits a sensor
        let exec_ctx_user1 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&user1, &[])));
        let res1 = contract.submit_sensor(exec_ctx_user1, sensor_json.clone());
        assert!(res1.is_ok());

        let sensor_json = serde_json::json!({"name": "owner 2 test"});

        // user2 submits a sensor
        let exec_ctx_user2 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&user2, &[])));
        let res2 = contract.submit_sensor(exec_ctx_user2, sensor_json.clone());
        assert!(res2.is_ok());

        // Verify owners saved correctly
        let sensor1 = SENSORS
            .load(deps.as_ref().storage, 1)
            .expect("Sensor 1 missing");
        let sensor2 = SENSORS
            .load(deps.as_ref().storage, 2)
            .expect("Sensor 2 missing");

        assert_eq!(sensor1.owner, user1);
        assert_eq!(sensor2.owner, user2);
    }

    #[test]
    fn submit_sensor_fails_on_duplicate_data() {
        let user = Addr::unchecked("alice");
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        // Instantiate the contract
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Prepare identical sensor data
        let sensor_json = serde_json::json!({"model": "abc-1", "type": "air"});

        // First submission should succeed
        let exec_ctx1 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
        let result1 = contract.submit_sensor(exec_ctx1, sensor_json.clone());
        assert!(result1.is_ok());

        // Second submission with same data should fail (duplicate)
        let exec_ctx2 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
        let result2 = contract.submit_sensor(exec_ctx2, sensor_json.clone());

        assert!(
            result2.is_err(),
            "Expected error on duplicate sensor submission"
        );

        let error = result2.unwrap_err().to_string();
        assert!(
            error.contains("Duplicate entry"),
            "Expected DuplicateData error, got: {error}"
        );
    }

    #[test]
    fn activate_sensor_succeeds_for_admin_and_proposed_sensor() {
        let admin = Addr::unchecked("admin");
        let user = Addr::unchecked("user");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        // Instantiate contract with admin
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert a proposed sensor owned by user
        let sensor = Sensor {
            id: 1,
            owner: user.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Proposed,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Create ExecCtx for admin activating sensor #1
        let activate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let res = contract.activate(activate_ctx, 1);

        assert!(res.is_ok(), "Expected activation to succeed for admin");

        // Check sensor status updated to Active
        let updated_sensor = SENSORS.load(deps.as_ref().storage, 1).unwrap();
        assert_eq!(updated_sensor.status, SensorStatus::Active);
    }

    #[test]
    fn activate_sensor_fails_for_non_admin() {
        let admin = Addr::unchecked("admin");
        let user = Addr::unchecked("user");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert proposed sensor owned by user
        let sensor = Sensor {
            id: 1,
            owner: user.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Proposed,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // ExecCtx for non-admin user tries to activate
        let non_admin_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
        let err = contract.activate(non_admin_ctx, 1).unwrap_err();

        // Expect Unauthorized error
        assert!(
            err.to_string().contains("Unauthorized"),
            "Expected Unauthorized error, got: {err}"
        );
    }

    #[test]
    fn activate_sensor_fails_if_already_active() {
        let admin = Addr::unchecked("admin");
        let user = Addr::unchecked("user");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert a sensor already Active
        let sensor = Sensor {
            id: 1,
            owner: user.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        let admin_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.activate(admin_ctx, 1).unwrap_err();

        // Expect AlreadyActivated error
        assert!(
            err.to_string().contains("Sensor is already activated"),
            "Expected AlreadyActivated error, got: {err}"
        );
    }

    #[test]
    fn activate_sensor_fails_if_sensor_not_found() {
        let admin = Addr::unchecked("admin");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let admin_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        // Try activating a sensor id that does not exist
        let err = contract.activate(admin_ctx, 999).unwrap_err();

        assert!(
            err.to_string().contains("not found"),
            "Expected NotFound error, got: {err}"
        );
    }

    #[test]
    fn deactivate_sensor_succeeds_for_admin() {
        let admin = Addr::unchecked("admin");
        let user = Addr::unchecked("user");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert a sensor (can be any status)
        let sensor = Sensor {
            id: 1,
            owner: user.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // deactivating sensor
        let deactivate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let res = contract.deactivate(deactivate_ctx, 1);

        assert!(res.is_ok(), "Expected deactivate to succeed for admin");

        // Check sensor status updated to Inactive
        let updated_sensor = SENSORS.load(deps.as_ref().storage, 1).unwrap();
        assert_eq!(updated_sensor.status, SensorStatus::Inactive);
    }

    #[test]
    fn deactivate_sensor_fails_for_non_admin() {
        let admin = Addr::unchecked("admin");
        let user = Addr::unchecked("user");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert a sensor
        let sensor = Sensor {
            id: 1,
            owner: user.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Non-admin user tries to deactivate
        let non_admin_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
        let err = contract.deactivate(non_admin_ctx, 1).unwrap_err();

        assert!(
            err.to_string().contains("Unauthorized"),
            "Expected Unauthorized error, got: {err}"
        );
    }

    #[test]
    fn deactivate_sensor_fails_if_sensor_not_found() {
        let admin = Addr::unchecked("admin");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let admin_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        // Try deactivating a sensor id that does not exist
        let err = contract.deactivate(admin_ctx, 999).unwrap_err();

        assert!(
            err.to_string().contains("not found"),
            "Expected NotFound error, got: {err}"
        );
    }

    #[test]
    fn delete_sensor_succeeds_for_admin() {
        let admin = Addr::unchecked("admin");
        let user = Addr::unchecked("user");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let sensor = Sensor {
            id: 1,
            owner: user.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Proposed,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Admin deletes it
        let delete_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let res = contract.delete(delete_ctx, 1);
        assert!(res.is_ok(), "Expected delete to succeed for admin");

        // Should not be found anymore
        let sensor_result = SENSORS.may_load(deps.as_ref().storage, 1).unwrap();
        assert!(sensor_result.is_none(), "Sensor should be deleted");
    }

    #[test]
    fn delete_sensor_fails_for_non_admin() {
        let admin = Addr::unchecked("admin");
        let user = Addr::unchecked("user");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let sensor = Sensor {
            id: 1,
            owner: user.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Proposed,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Non-admin tries to delete
        let non_admin_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
        let err = contract.delete(non_admin_ctx, 1).unwrap_err();

        assert!(
            err.to_string().contains("Unauthorized"),
            "Expected Unauthorized error, got: {err}"
        );
    }

    #[test]
    fn delete_sensor_fails_if_sensor_not_found() {
        let admin = Addr::unchecked("admin");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Admin tries to delete non-existent sensor
        let delete_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.delete(delete_ctx, 999).unwrap_err();

        assert!(
            err.to_string().contains("not found"),
            "Expected not found error, got: {err}"
        );
    }

    #[test]
    fn delete_sensor_removes_exactly_one() {
        let admin = Addr::unchecked("admin");
        let user = Addr::unchecked("user");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert two sensors
        for id in 1..=2 {
            let sensor = Sensor {
                id,
                owner: user.clone(),
                data_str: "dummy".to_string(),
                status: SensorStatus::Proposed,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, id, &sensor).unwrap();
        }

        // Admin deletes sensor 1
        let delete_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let res = contract.delete(delete_ctx, 1);
        assert!(res.is_ok(), "Expected delete to succeed");

        // Sensor 1 gone
        assert!(
            SENSORS
                .may_load(deps.as_ref().storage, 1)
                .unwrap()
                .is_none()
        );

        // Sensor 2 still present
        assert!(
            SENSORS
                .may_load(deps.as_ref().storage, 2)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn add_verifier_unauthorized() {
        let admin = "admin".into_addr();
        let not_admin = "not_admin".into_addr();
        let verifier = "verifier".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Try to add verifier from a non-admin
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&not_admin, &[])));

        let err = contract.add_verifier(exec_ctx, verifier).unwrap_err();
        assert!(
            err.to_string().contains("Unauthorized"),
            "Expected unauthorized error, got: {err}"
        );
    }

    #[test]
    fn add_verifier_success() {
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add verifier as the admin
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
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
    fn add_verifier_twice() {
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add verifier the first time (should succeed)
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.add_verifier(exec_ctx, verifier.clone()).unwrap();

        // Try adding the same verifier again (should fail)
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.add_verifier(exec_ctx, verifier).unwrap_err();

        assert!(
            err.to_string().contains("Verifier already exists"),
            "Expected VerifierAlreadyExists error, got: {err}"
        );
    }

    #[test]
    fn submit_data_accepts_valid_json() {
        let submitter = Addr::unchecked("submitter");
        let admin = Addr::unchecked("admin");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        // Set up instantiation context with admin
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Submit a sensor from the submitter
        let sensor_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        let sensor_data_str = r#"{"name": "field", "location": "field"}"#;
        let valid_json: Value = serde_json::from_str(sensor_data_str).unwrap();
        let submit_res = contract.submit_sensor(sensor_ctx, valid_json);

        assert!(submit_res.is_ok());

        // Activate the sensor with admin
        let activate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let activate_res = contract.activate(activate_ctx, 1);
        assert!(
            activate_res.is_ok(),
            "Expected sensor activation to succeed"
        );

        // Submit valid JSON data
        let valid_json_str = r#"{"temperature": 24.5, "location": "field"}"#;
        let valid_json: Value = serde_json::from_str(valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        let res = contract.submit_data(submit_ctx, 1, valid_json);

        assert!(res.is_ok(), "Expected success on valid JSON");
    }

    #[test]
    fn submit_data_rejects_invalid_json_value() {
        let submitter = "submitter".into_addr();
        let admin = Addr::unchecked("admin");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        let invalid_values = vec![
            serde_json::json!({}), // empty object
            serde_json::json!([]), // empty array
            serde_json::json!(""), // empty string
        ];

        for invalid_json in invalid_values {
            let exec_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
            let err = contract.submit_data(exec_ctx, 1, invalid_json).unwrap_err();

            assert!(
                err.to_string().contains("Data is not valid json"),
                "Expected InvalidJson error, got: {err}"
            );
        }
    }

    #[test]
    fn submit_data_duplicate_hash_rejected() {
        let submitter = "submitter".into_addr();
        let admin = Addr::unchecked("admin");

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        let valid_json_str = r#"{"temp": 22}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract
            .submit_data(exec_ctx, 1, valid_json.clone())
            .unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        let err = contract.submit_data(exec_ctx, 1, valid_json).unwrap_err();
        assert!(err.to_string().contains("Duplicate"));
    }

    #[test]
    fn submit_data_allowed_for_different_sensors() {
        let contract = CitizenScienceRegistry::new();
        let user = "user".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for id in [1, 2] {
            let sensor = Sensor {
                id,
                owner: user.clone(),
                data_str: "dummy".to_string(),
                status: SensorStatus::Active,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, id, &sensor).unwrap();
        }

        let json_data = serde_json::json!({ "temp": 25 });

        for sensor_id in [1, 2] {
            let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&user, &[])));
            let result = contract.submit_data(exec_ctx, sensor_id, json_data.clone());
            assert!(result.is_ok());
        }
    }

    #[test]
    fn submit_data_fails_for_inactive_sensor() {
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an inactive sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Inactive,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Prepare valid JSON data
        let valid_json_str = r#"{"temperature": 42}"#;
        let valid_json: Value = serde_json::from_str(valid_json_str).unwrap();

        // Attempt to submit data to inactive sensor
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        let err = contract.submit_data(submit_ctx, 1, valid_json).unwrap_err();

        assert_eq!(err, ContractError::SensorInactive.into());
    }

    #[test]
    fn verify_data_success() {
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Submit a sensor from the submitter
        let sensor_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        let sensor_data_str = r#"{"name": "field", "location": "field"}"#;
        let valid_json: Value = serde_json::from_str(sensor_data_str).unwrap();
        let submit_res = contract.submit_sensor(sensor_ctx, valid_json);

        assert!(submit_res.is_ok());

        // Activate the sensor with admin
        let activate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let activate_res = contract.activate(activate_ctx, 1);
        assert!(
            activate_res.is_ok(),
            "Expected sensor activation to succeed"
        );

        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        let valid_json_str = r#"{"data": "value"}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();

        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, 1, valid_json).unwrap();

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
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();
        let not_verifier = "not_verifier".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        let valid_json_str = r#"{"some": "data"}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, 1, valid_json).unwrap();

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
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Submit data
        let valid_json_str = r#"{"x": 42}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, 1, valid_json).unwrap();

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
    fn verify_data_fails_for_nonexistent_entry() {
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add verifier
        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        // Attempt to verify a non-existent entry (e.g. entry ID = 42)
        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        let err = contract
            .verify_data(verify_ctx, 42)
            .unwrap_err()
            .to_string();

        assert!(err.contains("not found"), "Unexpected error message: {err}");
    }

    #[test]
    fn reward_submitter_success() {
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        let valid_json_str = r#"{"valid": true}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, 1, valid_json).unwrap();

        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        contract.verify_data(verify_ctx, 1).unwrap();

        // Prepare reward funds
        let reward_funds = vec![coin(REWARD_AMOUNT, DEFAULT_DENOM)];

        // Reward submitter with funds
        let reward_info = message_info(&admin, &reward_funds);
        let reward_ctx = ExecCtx::from((deps.as_mut(), mock_env(), reward_info));

        let res = contract.reward_submitter(reward_ctx, 1).unwrap();

        assert!(
            res.attributes
                .iter()
                .any(|attr| attr.key == "action" && attr.value == "reward_submitter"),
            "Missing reward_submitter action attribute"
        );
        assert!(
            res.messages
                .iter()
                .any(|msg| matches!(msg.msg, Bank(BankMsg::Send { .. }))),
            "Missing bank send message"
        );
    }

    #[test]
    fn reward_submitter_not_verified() {
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let now = ctx.env.block.time.seconds();
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Submit data
        let valid_json_str = r#"{"raw": "data"}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, 1, valid_json).unwrap();

        // Try to reward without verification
        let reward_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.reward_submitter(reward_ctx, 1).unwrap_err();

        assert!(
            err.to_string().contains("Data entry not verified yet"),
            "Expected NotVerified error, got: {err}"
        );
    }

    #[test]
    fn reward_submitter_already_rewarded() {
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add verifier
        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Submit and verify
        let valid_json_str = r#"{"hello": "world"}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, 1, valid_json).unwrap();

        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        contract.verify_data(verify_ctx, 1).unwrap();

        // First reward call with enough funds (e.g. 1000 DEFAULT_DENOM if that's the reward amount)
        let reward_funds = vec![coin(REWARD_AMOUNT, DEFAULT_DENOM)];
        let reward_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&admin, &reward_funds),
        ));
        contract.reward_submitter(reward_ctx, 1).unwrap();

        // Second reward call with same sender (even with funds) should fail as already rewarded
        let reward_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&admin, &reward_funds),
        ));
        let err = contract.reward_submitter(reward_ctx, 1).unwrap_err();

        assert_eq!(err, ContractError::AlreadyRewarded.into());
    }

    #[test]
    fn reward_submitter_fails_without_funds() {
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add verifier
        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Submit and verify
        let valid_json_str = r#"{"hello": "world"}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, 1, valid_json).unwrap();

        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        contract.verify_data(verify_ctx, 1).unwrap();

        // Attempt to reward with no funds
        let reward_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.reward_submitter(reward_ctx, 1).unwrap_err();

        assert_eq!(err, ContractError::InvalidFunds.into());
    }

    #[test]
    fn reward_submitter_fails_for_non_admin() {
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();
        let non_admin = "random_user".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add verifier
        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Submit and verify
        let valid_json_str = r#"{"hello": "world"}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract.submit_data(submit_ctx, 1, valid_json).unwrap();

        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        contract.verify_data(verify_ctx, 1).unwrap();

        // Attempt to reward using a non-admin address
        let reward_funds = vec![coin(REWARD_AMOUNT, DEFAULT_DENOM)];
        let reward_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&non_admin, &reward_funds),
        ));
        let err = contract.reward_submitter(reward_ctx, 1).unwrap_err();

        assert_eq!(err, ContractError::Unauthorized.into());
    }

    #[test]
    fn reward_submitter_fails_for_nonexistent_entry() {
        let admin = "admin".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Attempt to reward a non-existent data entry (e.g. entry ID = 42)
        let reward_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&admin, &[coin(1000, DEFAULT_DENOM)]),
        ));
        let err = contract
            .reward_submitter(reward_ctx, 42)
            .unwrap_err()
            .to_string();

        assert!(err.contains("not found"), "Unexpected error message: {err}");
    }

    #[test]
    fn full_data_entry_lifecycle() {
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        // Instantiate contract
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add verifier
        let add_verifier_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .add_verifier(add_verifier_ctx, verifier.clone())
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Submit data by submitter
        let valid_json_str = r#"{"valid": true}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract
            .submit_data(submit_ctx, 1, valid_json.clone())
            .unwrap();

        // Verify data by verifier
        let verify_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&verifier, &[])));
        contract.verify_data(verify_ctx, 1).unwrap();

        // Reward submitter: send funds along with the message
        let reward_funds = vec![coin(REWARD_AMOUNT, DEFAULT_DENOM)];
        let reward_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&admin, &reward_funds),
        ));
        contract.reward_submitter(reward_ctx, 1).unwrap();

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
    fn get_sensor_returns_existing_sensor() {
        let contract = CitizenScienceRegistry::new();
        let admin = "admin".into_addr();

        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        let now = ctx.env.block.time.seconds();

        // Prepopulate a sensor in storage
        let sensor = Sensor {
            id: 1,
            owner: Addr::unchecked("user"),
            data_str: "{\"type\": \"temp\"}".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Query the sensor
        let result = contract.get_sensor(query_ctx, 1);

        assert!(result.is_ok(), "Expected get_sensor to succeed");
        let returned = result.unwrap();
        assert_eq!(returned.id, sensor.id);
        assert_eq!(returned.owner, sensor.owner);
        assert_eq!(returned.data_str, sensor.data_str);
        assert_eq!(returned.status, sensor.status);
    }

    #[test]
    fn get_sensor_returns_error_when_not_found() {
        let contract = CitizenScienceRegistry::new();
        let deps = mock_dependencies();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        let result = contract.get_sensor(query_ctx, 42);

        assert!(
            result.is_err(),
            "Expected get_sensor to fail for nonexistent sensor"
        );

        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"), "Unexpected error message: {err}");
    }

    #[test]
    fn list_sensors_empty_returns_empty_list() {
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&"admin".into_addr(), &[]),
        ));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_sensors(query_ctx, None, None, None, None)
            .unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn list_sensors_with_exact_page_boundary() {
        let contract = CitizenScienceRegistry::new();
        let owner = "user".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for i in 1..=4 {
            let sensor = Sensor {
                id: i,
                owner: owner.clone(),
                data_str: format!("{{\"sensor\": {i}}}"),
                status: SensorStatus::Proposed,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, i, &sensor).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let first_page = contract
            .list_sensors(query_ctx, None, Some(2), None, None)
            .unwrap();
        assert_eq!(first_page.len(), 2);
        assert_eq!(first_page[0].id, 1);
        assert_eq!(first_page[1].id, 2);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let second_page = contract
            .list_sensors(query_ctx, Some(2), Some(2), None, None)
            .unwrap();
        assert_eq!(second_page.len(), 2);
        assert_eq!(second_page[0].id, 3);
        assert_eq!(second_page[1].id, 4);
    }

    #[test]
    fn list_sensors_start_after_last_id_returns_empty() {
        let contract = CitizenScienceRegistry::new();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for i in 1..=3 {
            let sensor = Sensor {
                id: i,
                owner: owner.clone(),
                data_str: "some data".to_string(),
                status: SensorStatus::Proposed,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, i, &sensor).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_sensors(query_ctx, Some(3), Some(10), None, None)
            .unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn list_sensors_limit_caps_at_30() {
        let contract = CitizenScienceRegistry::new();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for i in 1..=50 {
            let sensor = Sensor {
                id: i,
                owner: owner.clone(),
                data_str: format!("{{\"sensor\": {i}}}"),
                status: SensorStatus::Active,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, i, &sensor).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_sensors(query_ctx, None, Some(100), None, None)
            .unwrap();

        assert_eq!(result.len(), 30);
    }

    #[test]
    fn list_sensors_defaults_to_10() {
        let contract = CitizenScienceRegistry::new();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for i in 1..=15 {
            let sensor = Sensor {
                id: i,
                owner: owner.clone(),
                data_str: format!("{{\"sensor\": {i}}}"),
                status: SensorStatus::Proposed,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, i, &sensor).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_sensors(query_ctx, None, None, None, None)
            .unwrap();

        assert_eq!(result.len(), 10);
    }

    #[test]
    fn list_sensors_returns_paginated_entries() {
        let contract = CitizenScienceRegistry::new();
        let owner = "user".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for i in 1..=3 {
            let sensor = Sensor {
                id: i,
                owner: owner.clone(),
                data_str: format!("{{\"sensor\": {i}}}"),
                status: SensorStatus::Active,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, i, &sensor).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let sensors = contract
            .list_sensors(query_ctx, None, Some(10), None, None)
            .unwrap();

        assert_eq!(sensors.len(), 3);
        assert_eq!(sensors[0].id, 1);
        assert_eq!(sensors[2].id, 3);
    }

    #[test]
    fn list_sensors_filters_by_owner() {
        let contract = CitizenScienceRegistry::new();
        let owner1 = "owner1".into_addr();
        let owner2 = "owner2".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner1, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add sensors for both owners
        for i in 1..=5 {
            let sensor = Sensor {
                id: i,
                owner: if i % 2 == 0 {
                    owner2.clone()
                } else {
                    owner1.clone()
                },
                data_str: format!("{{\"sensor\": {i}}}"),
                status: SensorStatus::Active,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, i, &sensor).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let owner1_sensors = contract
            .list_sensors(query_ctx, None, Some(10), Some(owner1.clone()), None)
            .unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let owner2_sensors = contract
            .list_sensors(query_ctx, None, Some(10), Some(owner2.clone()), None)
            .unwrap();

        assert_eq!(owner1_sensors.len(), 3);
        assert!(owner1_sensors.iter().all(|s| s.owner == owner1));

        assert_eq!(owner2_sensors.len(), 2);
        assert!(owner2_sensors.iter().all(|s| s.owner == owner2));
    }

    #[test]
    fn list_sensors_filters_by_status() {
        let contract = CitizenScienceRegistry::new();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for i in 1..=5 {
            let status = if i % 2 == 0 {
                SensorStatus::Proposed
            } else {
                SensorStatus::Active
            };
            let sensor = Sensor {
                id: i,
                owner: owner.clone(),
                data_str: format!("{{\"sensor\": {i}}}"),
                status,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, i, &sensor).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let active_sensors = contract
            .list_sensors(query_ctx, None, Some(10), None, Some(SensorStatus::Active))
            .unwrap();

        assert_eq!(active_sensors.len(), 3);
        assert!(
            active_sensors
                .iter()
                .all(|s| s.status == SensorStatus::Active)
        );

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let proposed_sensors = contract
            .list_sensors(
                query_ctx,
                None,
                Some(10),
                None,
                Some(SensorStatus::Proposed),
            )
            .unwrap();

        assert_eq!(proposed_sensors.len(), 2);
        assert!(
            proposed_sensors
                .iter()
                .all(|s| s.status == SensorStatus::Proposed)
        );
    }

    #[test]
    fn list_sensors_filters_by_owner_and_status() {
        let contract = CitizenScienceRegistry::new();
        let owner1 = "owner1".into_addr();
        let owner2 = "owner2".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner1, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for i in 1..=6 {
            let owner = if i <= 3 { &owner1 } else { &owner2 };
            let status = if i % 2 == 0 {
                SensorStatus::Proposed
            } else {
                SensorStatus::Active
            };
            let sensor = Sensor {
                id: i,
                owner: owner.clone(),
                data_str: format!("{{\"sensor\": {i}}}"),
                status,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, i, &sensor).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let owner2_proposed = contract
            .list_sensors(
                query_ctx,
                None,
                Some(10),
                Some(owner2.clone()),
                Some(SensorStatus::Proposed),
            )
            .unwrap();

        assert_eq!(owner2_proposed.len(), 2);
        assert_eq!(owner2_proposed[0].owner, owner2);
        assert_eq!(owner2_proposed[0].status, SensorStatus::Proposed);
    }

    #[test]
    fn count_sensors_by_owner_and_status() {
        let contract = CitizenScienceRegistry::new();
        let owner1 = "owner1".into_addr();
        let owner2 = "owner2".into_addr();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner1, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for i in 1..=5 {
            let owner = if i <= 3 { &owner1 } else { &owner2 };
            let status = if i % 2 == 0 {
                SensorStatus::Proposed
            } else {
                SensorStatus::Active
            };
            let sensor = Sensor {
                id: i,
                owner: owner.clone(),
                data_str: format!("{{\"sensor\": {i}}}"),
                status,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, i, &sensor).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        let total = contract.count_sensors(query_ctx, None, None).unwrap();
        assert_eq!(total, 5);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let owner1_total = contract
            .count_sensors(query_ctx, Some(owner1.clone()), None)
            .unwrap();
        assert_eq!(owner1_total, 3);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        let owner2_active = contract
            .count_sensors(query_ctx, Some(owner2.clone()), Some(SensorStatus::Active))
            .unwrap();
        assert_eq!(owner2_active, 1);
    }

    #[test]
    fn get_data_entry_success() {
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        let valid_json_str = r#"{"key": "value"}"#.to_string();
        let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();
        let submit_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        contract
            .submit_data(submit_ctx, 1, valid_json.clone())
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
    fn get_data_entry_not_found() {
        let admin = "admin".into_addr();
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        // Instantiate contract
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Query for a non-existent entry
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let err = contract.get_data_entry(query_ctx, 42).unwrap_err();

        assert!(
            err.to_string().contains("not found"),
            "Expected not found error, got: {err}"
        );
    }

    #[test]
    fn list_data_entries_empty_returns_empty_list() {
        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&"admin".into_addr(), &[]),
        ));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_data_entries(query_ctx, None, None, None, None)
            .unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn list_data_entries_with_exact_page_boundary() {
        let contract = CitizenScienceRegistry::new();
        let submitter = "submitter".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        for i in 1..=4 {
            let json = format!(r#"{{"entry": {i}}}"#);
            let value: Value = serde_json::from_str(&json).unwrap();
            let exec_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
            contract.submit_data(exec_ctx, 1, value).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let first_page = contract
            .list_data_entries(query_ctx, None, Some(2), None, None)
            .unwrap();
        assert_eq!(first_page.len(), 2);
        assert_eq!(first_page[0].id, 1);
        assert_eq!(first_page[1].id, 2);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let second_page = contract
            .list_data_entries(query_ctx, Some(2), Some(2), None, None)
            .unwrap();
        assert_eq!(second_page.len(), 2);
        assert_eq!(second_page[0].id, 3);
        assert_eq!(second_page[1].id, 4);
    }

    #[test]
    fn list_data_entries_start_after_last_id_returns_empty() {
        let contract = CitizenScienceRegistry::new();
        let submitter = "submitter".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        for i in 1..=3 {
            let json = format!(r#"{{"entry": {i}}}"#);
            let value: Value = serde_json::from_str(&json).unwrap();
            let exec_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
            contract.submit_data(exec_ctx, 1, value).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_data_entries(query_ctx, Some(3), Some(5), None, None)
            .unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn list_data_entries_limit_caps_at_30() {
        let contract = CitizenScienceRegistry::new();
        let submitter = "submitter".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        for i in 1..=50 {
            let json = format!(r#"{{"entry": {i}}}"#);
            let value: Value = serde_json::from_str(&json).unwrap();
            let exec_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
            contract.submit_data(exec_ctx, 1, value).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_data_entries(query_ctx, None, Some(999), None, None)
            .unwrap();

        assert_eq!(result.len(), 30);
    }

    #[test]
    fn list_data_entries_defaults_to_10() {
        let contract = CitizenScienceRegistry::new();
        let submitter = "submitter".into_addr();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        for i in 1..=15 {
            let json = format!(r#"{{"entry": {i}}}"#);
            let value: Value = serde_json::from_str(&json).unwrap();
            let exec_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
            contract.submit_data(exec_ctx, 1, value).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_data_entries(query_ctx, None, None, None, None)
            .unwrap();

        assert_eq!(result.len(), 10);
    }

    #[test]
    fn list_data_entries_returns_paginated_entries() {
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert an active sensor directly into storage
        let sensor = Sensor {
            id: 1,
            owner: submitter.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Submit 3 data entries
        for i in 0..3 {
            let valid_json_str = format!(r#"{{"entry": {i}}}"#);
            let valid_json: Value = serde_json::from_str(&valid_json_str).unwrap();
            let exec_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
            contract.submit_data(exec_ctx, 1, valid_json).unwrap();
        }

        // Query entries
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let entries = contract
            .list_data_entries(query_ctx, None, Some(10), None, None)
            .unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].id, 1);
        assert_eq!(entries[2].id, 3);
    }

    #[test]
    fn list_data_entries_by_sensor_id_only() {
        let contract = CitizenScienceRegistry::new();
        let submitter = "submitter".into_addr();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add 2 sensors
        for id in 1..=2 {
            let sensor = Sensor {
                id,
                owner: submitter.clone(),
                data_str: format!("dummy{id}").to_string(),
                status: SensorStatus::Active,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, id, &sensor).unwrap();
        }

        // Submit 3 entries for sensor 1, 2 for sensor 2
        for i in 0..3 {
            let val: Value = serde_json::json!({ "entry": i });
            let exec_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
            contract.submit_data(exec_ctx, 1, val).unwrap();
        }
        for i in 0..2 {
            let val: Value = serde_json::json!({ "entry": i });
            let exec_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&submitter, &[])));
            contract.submit_data(exec_ctx, 2, val).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_data_entries(query_ctx, None, None, None, Some(2))
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|entry| entry.sensor_id == 2));
    }

    #[test]
    fn list_data_entries_by_submitter_only() {
        let contract = CitizenScienceRegistry::new();
        let alice = "alice".into_addr();
        let bob = "bob".into_addr();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&alice, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let sensor = Sensor {
            id: 1,
            owner: alice.clone(),
            data_str: "dummy".to_string(),
            status: SensorStatus::Active,
            created_at: now,
            updated_at: now,
        };
        SENSORS.save(deps.as_mut().storage, 1, &sensor).unwrap();

        // Alice submits 3 entries
        for i in 0..3 {
            let val: Value = serde_json::json!({ "entry": i });
            let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&alice, &[])));
            contract.submit_data(exec_ctx, 1, val).unwrap();
        }

        // Bob submits 2 entries
        for i in 3..5 {
            let val: Value = serde_json::json!({ "entry": i });
            let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&bob, &[])));
            contract.submit_data(exec_ctx, 1, val).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_data_entries(query_ctx, None, None, Some(bob.clone()), None)
            .unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|entry| entry.submitter == bob));
    }

    #[test]
    fn list_data_entries_by_sensor_id_and_submitter() {
        let contract = CitizenScienceRegistry::new();
        let alice = "alice".into_addr();
        let bob = "bob".into_addr();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&alice, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for id in 1..=2 {
            let sensor = Sensor {
                id,
                owner: alice.clone(),
                data_str: "dummy".to_string(),
                status: SensorStatus::Active,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, id, &sensor).unwrap();
        }

        // Bob submits to sensor 1 and 2
        for sensor_id in [1, 2] {
            let val: Value = serde_json::json!({ "entry": sensor_id });
            let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&bob, &[])));
            contract.submit_data(exec_ctx, sensor_id, val).unwrap();
        }

        // Alice submits to sensor 1
        let val: Value = serde_json::json!({ "entry": 99 });
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&alice, &[])));
        contract.submit_data(exec_ctx, 1, val).unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .list_data_entries(query_ctx, None, None, Some(bob.clone()), Some(1))
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].submitter, bob);
        assert_eq!(result[0].sensor_id, 1);
    }

    #[test]
    fn count_data_entries_by_submitter_and_sensor_id() {
        let contract = CitizenScienceRegistry::new();
        let alice = "alice".into_addr();
        let bob = "bob".into_addr();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&alice, &[])));
        let now = ctx.env.block.time.seconds();

        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for id in 1..=2 {
            let sensor = Sensor {
                id,
                owner: alice.clone(),
                data_str: "dummy".to_string(),
                status: SensorStatus::Active,
                created_at: now,
                updated_at: now,
            };
            SENSORS.save(deps.as_mut().storage, id, &sensor).unwrap();
        }

        // Bob submits 2 entries
        for sensor_id in [1, 2] {
            let val: Value = serde_json::json!({ "entry": sensor_id });
            let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&bob, &[])));
            contract.submit_data(exec_ctx, sensor_id, val).unwrap();
        }

        // Alice submits 1 entry to sensor 1
        let val: Value = serde_json::json!({ "entry": 999 });
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&alice, &[])));
        contract.submit_data(exec_ctx, 1, val).unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        let total = contract.count_data_entries(query_ctx, None, None).unwrap();
        assert_eq!(total, 3);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        let bob_entries = contract
            .count_data_entries(query_ctx, Some(bob.clone()), None)
            .unwrap();
        assert_eq!(bob_entries, 2);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        let alice_sensor1 = contract
            .count_data_entries(query_ctx, Some(alice.clone()), Some(1))
            .unwrap();
        assert_eq!(alice_sensor1, 1);
    }

    #[test]
    fn list_verifiers_returns_all_registered_addresses() {
        let admin = "admin".into_addr();
        let verifier1 = "verifier1".into_addr();
        let verifier2 = "verifier2".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add verifiers
        let ctx1 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.add_verifier(ctx1, verifier1.clone()).unwrap();

        let ctx2 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.add_verifier(ctx2, verifier2.clone()).unwrap();

        // Query verifiers
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let verifiers = contract.list_verifiers(query_ctx).unwrap();

        assert_eq!(verifiers.len(), 2);
        assert!(verifiers.contains(&verifier1));
        assert!(verifiers.contains(&verifier2));
    }

    #[test]
    fn list_verifiers_empty() {
        let admin = "admin".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let verifiers = contract.list_verifiers(query_ctx).unwrap();

        assert!(verifiers.is_empty());
    }

    #[test]
    fn is_verifier_returns_true_for_registered_and_false_for_unregistered() {
        let admin = "admin".into_addr();
        let verifier1 = "verifier1".into_addr();
        let verifier2 = "verifier2".into_addr();

        let contract = CitizenScienceRegistry::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Add verifier1 as the admin
        let ctx1 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
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
