use crate::constants::{
    ADMIN, DATA_HASHES, DEFAULT_DENOM, DENOM, DONATIONS, NEXT_PROJECT_ID, PROJECTS,
};
use crate::enums::ProjectStatus;
use crate::errors::ContractError;
#[cfg(test)]
use chrono::Utc;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::Bound;
#[cfg(test)]
use serde_json::json;
use serde_json::{Value, to_string};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_std::{Addr, BankMsg, Coin, Order, Response, StdResult, Uint128};
use sylvia::entry_points;

#[cw_serde]
pub struct Project {
    pub id: u64,
    pub owner: Addr,
    pub goal: Uint128,
    pub data_str: String,
    pub total_donated: Uint128,
    pub status: ProjectStatus,
}

pub struct WaterWellInitiativeContract;

#[cfg_attr(not(feature = "library"), entry_points)]
#[contract]
#[sv::error(ContractError)]
impl WaterWellInitiativeContract {
    pub const fn new() -> Self {
        Self
    }

    #[sv::msg(instantiate)]
    fn instantiate(&self, ctx: InstantiateCtx, denom: Option<String>) -> StdResult<Response> {
        ADMIN.save(ctx.deps.storage, &ctx.info.sender)?;
        NEXT_PROJECT_ID.save(ctx.deps.storage, &1)?;

        let denom_to_store = denom.unwrap_or_else(|| DEFAULT_DENOM.to_string());
        DENOM.save(ctx.deps.storage, &denom_to_store)?;

        Ok(Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("denom", denom_to_store))
    }

    #[sv::msg(exec)]
    fn create_project(&self, ctx: ExecCtx, goal: Uint128, data: Value) -> StdResult<Response> {
        if goal.is_zero() {
            return Err(ContractError::ZeroGoal.into());
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
        let data_str = to_string(&data).map_err(|_| ContractError::InvalidJson)?;

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

        let id = NEXT_PROJECT_ID.load(ctx.deps.storage)?;
        let project = Project {
            id,
            owner: ctx.info.sender.clone(),
            goal,
            data_str: data_str.clone(),
            total_donated: Uint128::zero(),
            status: ProjectStatus::default(),
        };

        DATA_HASHES.save(ctx.deps.storage, data_hash.clone(), &true)?;
        PROJECTS.save(ctx.deps.storage, id, &project)?;
        NEXT_PROJECT_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "create_project")
            .add_attribute("project_id", id.to_string())
            .add_attribute("status", format!("{:?}", project.status)))
    }

    #[sv::msg(exec)]
    fn cancel(&self, ctx: ExecCtx, project_id: u64) -> StdResult<Response> {
        let admin = ADMIN.load(ctx.deps.storage)?;
        let mut project = PROJECTS.load(ctx.deps.storage, project_id)?;

        let is_owner = ctx.info.sender == project.owner;
        let is_admin = ctx.info.sender == admin;

        match project.status {
            ProjectStatus::Cancelled => {
                return Err(ContractError::AlreadyCancelled.into());
            }

            ProjectStatus::Proposed => {
                // ! Only the owner or admin can cancel a project in Proposed state.
                if !is_owner && !is_admin {
                    return Err(ContractError::Unauthorized.into());
                }
            }

            ProjectStatus::Fundraising => {
                // ! Only admin can cancel a project in Fundraising state.
                if !is_admin {
                    return Err(ContractError::Unauthorized.into());
                }
            }

            _ => return Err(ContractError::CannotCancel.into()),
        }

        project.status = ProjectStatus::Cancelled;
        PROJECTS.save(ctx.deps.storage, project_id, &project)?;

        Ok(Response::new()
            .add_attribute("action", "cancel")
            .add_attribute("project_id", project_id.to_string())
            .add_attribute("cancelled_by", ctx.info.sender.to_string())
            .add_attribute("status", format!("{:?}", project.status)))
    }

    #[sv::msg(exec)]
    fn validate(&self, ctx: ExecCtx, project_id: u64) -> StdResult<Response> {
        let admin = ADMIN.load(ctx.deps.storage)?;
        let sender = &ctx.info.sender;
        if sender != admin {
            return Err(ContractError::Unauthorized.into());
        }

        let mut project = PROJECTS.load(ctx.deps.storage, project_id)?;

        match project.status {
            ProjectStatus::Cancelled => return Err(ContractError::Cancelled.into()),
            ProjectStatus::Proposed => {
                project.status = ProjectStatus::Fundraising;
            }
            _ => return Err(ContractError::AlreadyValidated.into()),
        }

        PROJECTS.save(ctx.deps.storage, project_id, &project)?;

        Ok(Response::new()
            .add_attribute("action", "validate")
            .add_attribute("admin", admin.to_string())
            .add_attribute("owner", project.owner.to_string())
            .add_attribute("project_id", project_id.to_string())
            .add_attribute("status", format!("{:?}", project.status)))
    }

    #[sv::msg(exec)]
    fn donate(&self, ctx: ExecCtx, project_id: u64) -> StdResult<Response> {
        let mut project = PROJECTS.load(ctx.deps.storage, project_id)?;

        match project.status {
            ProjectStatus::Cancelled => return Err(ContractError::Cancelled.into()),
            ProjectStatus::Funded => return Err(ContractError::AlreadyFunded.into()),
            ProjectStatus::Completed => return Err(ContractError::AlreadyCompleted.into()),
            ProjectStatus::Fundraising => {} // allowed
            _ => return Err(ContractError::NotValidated.into()),
        }

        let stored_denom = DENOM.load(ctx.deps.storage)?;

        let donation = ctx
            .info
            .funds
            .iter()
            .find(|c| c.denom == stored_denom)
            .map(|c| c.amount)
            .unwrap_or_default();

        if donation.is_zero() {
            return Err(ContractError::NoDonation.into());
        }

        if project.total_donated + donation > project.goal {
            return Err(ContractError::ExceedGoal.into());
        }

        project.total_donated += donation;

        if project.total_donated == project.goal {
            project.status = ProjectStatus::Funded;
        }

        PROJECTS.save(ctx.deps.storage, project_id, &project)?;

        let donor = &ctx.info.sender;

        let prev = DONATIONS
            .may_load(ctx.deps.storage, (project_id, donor))?
            .unwrap_or_default();

        DONATIONS.save(ctx.deps.storage, (project_id, donor), &(prev + donation))?;

        Ok(Response::new()
            .add_attribute("action", "donate")
            .add_attribute("project_id", project_id.to_string())
            .add_attribute("donor", donor.to_string())
            .add_attribute("amount", donation.to_string())
            .add_attribute("status", format!("{:?}", project.status)))
    }

    #[sv::msg(exec)]
    fn unlock(&self, ctx: ExecCtx, project_id: u64) -> StdResult<Response> {
        let admin = ADMIN.load(ctx.deps.storage)?;
        let sender = &ctx.info.sender;
        if sender != admin {
            return Err(ContractError::Unauthorized.into());
        }
        let mut project = PROJECTS.load(ctx.deps.storage, project_id)?;

        match project.status {
            ProjectStatus::Cancelled => return Err(ContractError::Cancelled.into()),
            ProjectStatus::Funded => {
                project.status = ProjectStatus::Disbursable;
            }
            ProjectStatus::Disbursable => return Err(ContractError::AlreadyDisbursable.into()),
            ProjectStatus::Completed => return Err(ContractError::AlreadyCompleted.into()),
            _ => return Err(ContractError::GoalNotMet.into()),
        }

        PROJECTS.save(ctx.deps.storage, project_id, &project)?;

        Ok(Response::new()
            .add_attribute("action", "unlock")
            .add_attribute("admin", admin.to_string())
            .add_attribute("owner", project.owner.to_string())
            .add_attribute("project_id", project_id.to_string())
            .add_attribute("status", format!("{:?}", project.status)))
    }

    #[sv::msg(exec)]
    fn disburse(&self, ctx: ExecCtx, project_id: u64) -> StdResult<Response> {
        let mut project = PROJECTS.load(ctx.deps.storage, project_id)?;

        match project.status {
            ProjectStatus::Cancelled => return Err(ContractError::Cancelled.into()),
            ProjectStatus::Completed => return Err(ContractError::AlreadyCompleted.into()),
            ProjectStatus::Disbursable => {} // OK to proceed
            _ => return Err(ContractError::NotDisbursable.into()),
        }

        let sender = &ctx.info.sender;
        let admin = ADMIN.load(ctx.deps.storage)?;
        if sender != &project.owner && sender != &admin {
            return Err(ContractError::Unauthorized.into());
        }

        project.status = ProjectStatus::Completed;
        PROJECTS.save(ctx.deps.storage, project_id, &project)?;

        let stored_denom = DENOM.load(ctx.deps.storage)?;

        let send_msg = BankMsg::Send {
            to_address: project.owner.to_string(),
            amount: vec![Coin {
                denom: stored_denom.to_string(),
                amount: project.total_donated,
            }],
        };

        Ok(Response::new()
            .add_message(send_msg)
            .add_attribute("action", "disburse")
            .add_attribute("project_id", project_id.to_string())
            .add_attribute("sender", sender.to_string())
            .add_attribute("recipient", project.owner.to_string())
            .add_attribute("amount", project.total_donated.to_string())
            .add_attribute("status", format!("{:?}", project.status)))
    }

    #[sv::msg(exec)]
    fn refund(&self, ctx: ExecCtx, project_id: u64) -> StdResult<Response> {
        let project = PROJECTS.load(ctx.deps.storage, project_id)?;

        if project.status != ProjectStatus::Cancelled {
            return Err(ContractError::NotRefundable.into());
        }

        let donor = &ctx.info.sender;
        let amount = DONATIONS
            .may_load(ctx.deps.storage, (project_id, donor))?
            .unwrap_or_default();

        if amount.is_zero() {
            return Err(ContractError::NoRefundAvailable.into());
        }

        // Remove refund so it can’t be double-claimed
        DONATIONS.remove(ctx.deps.storage, (project_id, donor));

        let stored_denom = DENOM.load(ctx.deps.storage)?;

        Ok(Response::new()
            .add_message(BankMsg::Send {
                to_address: donor.to_string(),
                amount: vec![Coin {
                    denom: stored_denom.to_string(),
                    amount,
                }],
            })
            .add_attribute("action", "refund")
            .add_attribute("project_id", project_id.to_string())
            .add_attribute("refunded_to", donor.to_string())
            .add_attribute("amount", amount.to_string())
            .add_attribute("status", format!("{:?}", project.status)))
    }

    #[sv::msg(query)]
    fn get_project(&self, ctx: QueryCtx, project_id: u64) -> StdResult<Project> {
        PROJECTS.load(ctx.deps.storage, project_id)
    }

    #[sv::msg(query)]
    fn list_projects(
        &self,
        ctx: QueryCtx,
        limit: Option<u32>,
        start_after: Option<u64>,
    ) -> StdResult<Vec<Project>> {
        let limit = limit.unwrap_or(10).min(30) as usize;

        let start = start_after.map(Bound::exclusive);

        PROJECTS
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| item.map(|(_, p)| p))
            .collect()
    }

    #[sv::msg(query)]
    fn get_project_status_counts(&self, ctx: QueryCtx) -> StdResult<HashMap<String, u32>> {
        // Initialize with all variants and 0 counts
        let mut result: HashMap<String, u32> = [
            ProjectStatus::Proposed,
            ProjectStatus::Fundraising,
            ProjectStatus::Funded,
            ProjectStatus::Disbursable,
            ProjectStatus::Completed,
            ProjectStatus::Cancelled,
        ]
        .iter()
        .map(|status| (status.to_string(), 0))
        .collect();

        for item in PROJECTS.range(ctx.deps.storage, None, None, Order::Ascending) {
            let (_, project) = item?;
            let key = project.status.to_string();
            *result.entry(key).or_insert(0) += 1;
        }

        Ok(result)
    }

    #[sv::msg(query)]
    fn get_projects_by_status(
        &self,
        ctx: QueryCtx,
        status: ProjectStatus,
        limit: Option<u32>,
        start_after: Option<u64>,
    ) -> StdResult<Vec<Project>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);

        PROJECTS
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .filter_map(|item| {
                let (_, project) = item.ok()?;
                if project.status == status {
                    Some(Ok(project))
                } else {
                    None
                }
            })
            .take(limit)
            .collect()
    }
}

/// Returns a valid project JSON string with a dynamic timestamp in ISO 8601 format.
#[cfg(test)]
pub fn valid_json_str() -> String {
    let now = Utc::now().to_rfc3339();

    let value = json!({
        "name": "Clean Water for Village A",
        "location": "3.456, -76.532",
        "details": {
            "description": "Installation of a solar-powered water pump to serve the local community.",
            "images": [
                "https://example.com/image1.jpg",
                "https://example.com/image2.jpg"
            ],
            "beneficiaries": 300,
            "timeline": "2025-07 to 2025-12",
            "created_at": now
        }
    });

    value.to_string()
}

#[cfg(test)]
mod tests {
    use crate::constants::DEFAULT_DENOM;
    use crate::enums::ProjectStatus;

    use super::*;
    use sylvia::cw_multi_test::IntoAddr;
    use sylvia::cw_std::StdError;
    use sylvia::cw_std::testing::{message_info, mock_dependencies, mock_env};

    #[test]
    fn instantiate() {
        let admin = "admin".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let stored_admin = ADMIN.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_admin, admin);

        let stored_next_id = NEXT_PROJECT_ID.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_next_id, 1);
    }

    #[test]
    fn instantiate_defaults_to_default_denom_if_not_provided() {
        let admin = "admin".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));

        // Call instantiate without providing a denom
        contract.instantiate(ctx, None).unwrap();

        // Load and assert the default denom is used
        let stored_denom = DENOM.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_denom, DEFAULT_DENOM.to_string());

        // Check admin saved correctly
        let stored_admin = ADMIN.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_admin, admin);

        // Check the next project ID initialized correctly
        let stored_next_id = NEXT_PROJECT_ID.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_next_id, 1);
    }

    #[test]
    fn query_nonexistent_project() {
        let admin = "admin".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let err = contract.get_project(ctx, 1).unwrap_err();

        match err {
            StdError::NotFound { .. } => {}
            _ => panic!("Expected NotFound error, got: {err}"),
        }

        let ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let err = contract.get_project(ctx, 1).unwrap_err();

        match err {
            StdError::NotFound { .. } => {}
            _ => panic!("Expected NotFound error, got: {err}"),
        }
    }

    #[test]
    fn query_existing_project() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let goal = Uint128::from(1000u128);
        let exec_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(
                &owner,
                &[Coin {
                    denom: DEFAULT_DENOM.to_string(),
                    amount: Uint128::zero(),
                }],
            ),
        ));
        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract.create_project(exec_ctx, goal, valid_json).unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let project = contract.get_project(query_ctx, 1).unwrap();

        assert_eq!(project.id, 1);
        assert_eq!(project.owner, owner);
        assert_eq!(project.goal, goal);
        assert!(project.status != ProjectStatus::Fundraising);
        assert_eq!(project.total_donated, Uint128::zero());
    }

    #[test]
    fn create_project() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Attempt to create project with a non-admin sender
        let exec_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(
                &owner,
                &[Coin {
                    denom: DEFAULT_DENOM.to_string(),
                    amount: Uint128::zero(),
                }],
            ),
        ));

        let goal = Uint128::from(1000u128);

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();
        contract.create_project(exec_ctx, goal, valid_json).unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let project = contract.get_project(query_ctx, 1).unwrap();

        assert_eq!(project.id, 1);
        assert_eq!(project.owner, owner);
        assert_eq!(project.goal, goal);
        assert!(project.status != ProjectStatus::Fundraising);
        assert_eq!(project.total_donated, Uint128::zero());
    }

    #[test]
    fn get_project() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        // Instanciation du contrat
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Création du projet par le owner
        let exec_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(
                &owner,
                &[Coin {
                    denom: DEFAULT_DENOM.to_string(),
                    amount: Uint128::zero(),
                }],
            ),
        ));

        let goal = Uint128::from(1000u128);
        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract.create_project(exec_ctx, goal, valid_json).unwrap();

        // Query pour récupérer le projet créé
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let project = contract.get_project(query_ctx, 1).unwrap();

        // Assertions
        assert_eq!(project.id, 1);
        assert_eq!(project.owner, owner);
        assert_eq!(project.goal, goal);
        assert_eq!(project.total_donated, Uint128::zero());
        assert_eq!(project.status, ProjectStatus::Proposed);
    }

    #[test]
    fn create_project_fail_zero_goal() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let err = contract
            .create_project(exec_ctx, Uint128::zero(), valid_json)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Generic error: Goal must be greater than zero"
        );
    }

    #[test]
    fn create_multiple_projects_increments_id() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        contract
            .instantiate(
                InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                Some(DEFAULT_DENOM.to_string()),
            )
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        // First project
        contract
            .create_project(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[]))),
                Uint128::from(1000u128),
                valid_json.clone(),
            )
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        // Second project
        contract
            .create_project(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[]))),
                Uint128::from(2000u128),
                valid_json,
            )
            .unwrap();

        // Query second project to verify ID is 2
        let project = contract
            .get_project(QueryCtx::from((deps.as_ref(), mock_env())), 2)
            .unwrap();
        assert_eq!(project.id, 2);
        assert_eq!(project.goal, Uint128::from(2000u128));
    }

    #[test]
    fn create_project_rejects_invalid_json_value() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        contract
            .instantiate(
                InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                Some(DEFAULT_DENOM.to_string()),
            )
            .unwrap();

        let invalid_values = vec![
            serde_json::json!({}), // empty object
            serde_json::json!([]), // empty array
            serde_json::json!(""), // empty string
        ];

        for invalid_json in invalid_values {
            let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
            let err = contract
                .create_project(exec_ctx, Uint128::from(1000u128), invalid_json)
                .unwrap_err();

            assert!(
                err.to_string().contains("InvalidJson")
                    || err.to_string().contains("Data is not valid json"),
                "Expected InvalidJson error, got: {err}"
            );
        }
    }

    #[test]
    fn create_project_duplicate_hash_rejected() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        contract
            .instantiate(
                InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                Some(DEFAULT_DENOM.to_string()),
            )
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        // First creation should succeed
        contract
            .create_project(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[]))),
                Uint128::from(1000u128),
                valid_json.clone(),
            )
            .unwrap();

        // Second creation with identical data should fail
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let err = contract
            .create_project(exec_ctx, Uint128::from(1000u128), valid_json)
            .unwrap_err();

        assert!(
            err.to_string().contains("Duplicate"),
            "Expected duplicate hash rejection, got: {err}"
        );
    }

    #[test]
    fn validate_project_success() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let create_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(
                &owner,
                &[Coin {
                    denom: DEFAULT_DENOM.to_string(),
                    amount: Uint128::zero(),
                }],
            ),
        ));

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(create_ctx, Uint128::from(1000u128), valid_json)
            .unwrap();

        // Validate project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let res = contract.validate(validate_ctx, 1).unwrap();

        // Verify response attributes
        let attrs: std::collections::HashMap<_, _> = res
            .attributes
            .iter()
            .map(|attr| (attr.key.as_str(), attr.value.as_str()))
            .collect();

        assert_eq!(attrs.get("action"), Some(&"validate"));
        assert_eq!(attrs.get("admin"), Some(&admin.as_str()));
        assert_eq!(attrs.get("owner"), Some(&owner.as_str()));
        assert_eq!(attrs.get("project_id"), Some(&"1"));

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let project = contract.get_project(query_ctx, 1).unwrap();
        assert!(project.status == ProjectStatus::Fundraising);
    }

    #[test]
    fn validate_unauthorized_fails() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let create_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(
                &owner,
                &[Coin {
                    denom: DEFAULT_DENOM.to_string(),
                    amount: Uint128::zero(),
                }],
            ),
        ));

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(create_ctx, Uint128::from(500u128), valid_json)
            .unwrap();

        // Attempt validation from non-admin
        let invalid_validate_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let err = contract.validate(invalid_validate_ctx, 1).unwrap_err();

        assert_eq!(err.to_string(), "Generic error: Unauthorized");
    }

    #[test]
    fn validate_fails_already_validated() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let create_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(create_ctx, Uint128::from(1000u128), valid_json)
            .unwrap();

        // First validation succeeds
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Second validation should fail
        let second_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.validate(second_ctx, 1).unwrap_err();

        assert_eq!(
            err.to_string(),
            "Generic error: Project is already validated"
        );
    }

    #[test]
    fn cancel_project_success_proposed() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        // Instantiate contract
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Create project
        let create_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(
                &owner,
                &[Coin {
                    denom: DEFAULT_DENOM.to_string(),
                    amount: Uint128::zero(),
                }],
            ),
        ));

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();
        contract
            .create_project(create_ctx, Uint128::from(1000u128), valid_json)
            .unwrap();

        // Cancel project (should be in Proposed state)
        let cancel_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let res = contract.cancel(cancel_ctx, 1).unwrap();

        let attrs: std::collections::HashMap<_, _> = res
            .attributes
            .iter()
            .map(|attr| (attr.key.as_str(), attr.value.as_str()))
            .collect();

        assert_eq!(attrs.get("action"), Some(&"cancel"));
        assert_eq!(attrs.get("status"), Some(&"Cancelled"));
    }

    #[test]
    fn cancel_project_success_fundraising() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        // Instantiate
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Create project
        let create_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();
        contract
            .create_project(create_ctx, Uint128::from(1000u128), valid_json)
            .unwrap();

        // Validate (moves to Fundraising)
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Cancel in Fundraising
        let cancel_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let res = contract.cancel(cancel_ctx, 1).unwrap();

        let attrs: std::collections::HashMap<_, _> = res
            .attributes
            .iter()
            .map(|attr| (attr.key.as_str(), attr.value.as_str()))
            .collect();

        assert_eq!(attrs.get("action"), Some(&"cancel"));
        assert_eq!(attrs.get("status"), Some(&"Cancelled"));
    }

    #[test]
    fn cancel_project_by_owner_fails_in_fundraising() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        contract
            .instantiate(
                InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                Some(DEFAULT_DENOM.to_string()),
            )
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();
        contract
            .create_project(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[]))),
                Uint128::from(1000u128),
                valid_json,
            )
            .unwrap();

        // Validate (to Fundraising)
        contract
            .validate(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                1,
            )
            .unwrap();

        // Owner tries to cancel in Fundraising (should fail)
        let err = contract
            .cancel(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[]))),
                1,
            )
            .unwrap_err();

        assert_eq!(err.to_string(), "Generic error: Unauthorized");
    }

    #[test]
    fn cancel_project_fails_unauthorized_user() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let stranger = "stranger".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        contract
            .instantiate(
                InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                Some(DEFAULT_DENOM.to_string()),
            )
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();
        contract
            .create_project(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[]))),
                Uint128::from(1000u128),
                valid_json,
            )
            .unwrap();

        // Stranger (not owner/admin) tries to cancel in Proposed
        let err = contract
            .cancel(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&stranger, &[]))),
                1,
            )
            .unwrap_err();

        assert_eq!(err.to_string(), "Generic error: Unauthorized");
    }

    #[test]
    fn cancel_project_fails_if_already_cancelled() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        contract
            .instantiate(
                InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                Some(DEFAULT_DENOM.to_string()),
            )
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();
        contract
            .create_project(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[]))),
                Uint128::from(1000u128),
                valid_json,
            )
            .unwrap();

        // Cancel once
        contract
            .cancel(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                1,
            )
            .unwrap();

        // Cancel again (should fail)
        let err = contract
            .cancel(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                1,
            )
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "Generic error: Project is already cancelled"
        );
    }

    #[test]
    fn cancel_project_fails_if_funded_or_completed_cheat() {
        let admin = "admin".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        contract
            .instantiate(
                InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                Some(DEFAULT_DENOM.to_string()),
            )
            .unwrap();

        // Funded project
        let funded_project = Project {
            id: 1,
            owner: admin.clone(),
            goal: Uint128::from(1000u128),
            total_donated: Uint128::from(1000u128),
            status: ProjectStatus::Funded,
            data_str: valid_json_str().to_string(),
        };

        PROJECTS
            .save(deps.as_mut().storage, 1, &funded_project)
            .unwrap();

        // Admin tries to cancel a Funded project
        let err_funded = contract
            .cancel(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                1,
            )
            .unwrap_err();

        assert_eq!(
            err_funded.to_string(),
            "Generic error: Project cannot be cancelled in its current status"
        );

        // Completed project
        let mut completed_project = funded_project.clone();
        completed_project.status = ProjectStatus::Completed;
        PROJECTS
            .save(deps.as_mut().storage, 1, &completed_project)
            .unwrap();

        let err_completed = contract
            .cancel(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                1,
            )
            .unwrap_err();

        assert_eq!(
            err_completed.to_string(),
            "Generic error: Project cannot be cancelled in its current status"
        );
    }

    #[test]
    fn cancel_fails_nonexistent_project() {
        let admin = "admin".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        contract
            .instantiate(
                InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                Some(DEFAULT_DENOM.to_string()),
            )
            .unwrap();

        let err = contract
            .cancel(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                999, // non-existent project_id
            )
            .unwrap_err();

        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn donate_success() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Donor sends 500 uatom
        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(500),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));

        let res = contract.donate(exec_ctx, 1).unwrap();
        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "action" && a.value == "donate")
        );

        let project = PROJECTS.load(&deps.storage, 1).unwrap();

        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "action" && a.value == "donate")
        );
        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "project_id" && a.value == "1")
        );
        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "donor" && a.value == donor.to_string())
        );
        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "amount" && a.value == "500")
        );

        assert_eq!(project.total_donated, Uint128::new(500));
        assert!(project.status == ProjectStatus::Fundraising);
    }

    #[test]
    fn donate_fails_wrong_denom() {
        let admin = "admin".into_addr();
        let donor = "donor".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let create_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(create_ctx, Uint128::from(1000u128), valid_json)
            .unwrap();

        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        let bad_funds = vec![Coin {
            denom: "uatom".to_string(),
            amount: Uint128::from(500u128),
        }];

        let donate_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &bad_funds)));
        let err = contract.donate(donate_ctx, 1).unwrap_err();

        assert_eq!(err.to_string(), "Generic error: Unvalid donation");
    }

    #[test]
    fn donate_not_validated_fails() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Donor sends funds on non validated project
        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(100),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));

        let err = contract.donate(exec_ctx, 1).unwrap_err();
        assert_eq!(err.to_string(), "Generic error: Project is not validated");

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert_eq!(project.total_donated, Uint128::new(0));
        assert!(project.status == ProjectStatus::Proposed);
    }

    #[test]
    fn donate_no_funds_fails() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Donor sends no funds
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &[])));

        let err = contract.donate(exec_ctx, 1).unwrap_err();
        assert_eq!(err.to_string(), "Generic error: Unvalid donation");
        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert_eq!(project.total_donated, Uint128::new(0));
        assert!(project.status == ProjectStatus::Fundraising);
    }

    #[test]
    fn donate_exceed_goal_fails() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Donor sends 1200 uatom (exceeds goal)
        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(1200),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));

        let err = contract.donate(exec_ctx, 1).unwrap_err();
        assert_eq!(err.to_string(), "Generic error: Donation exceeds goal");

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert_eq!(project.total_donated, Uint128::new(0));
        assert!(project.status == ProjectStatus::Fundraising);
    }

    #[test]
    fn donate_multiple_partial() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor1 = "donor1".into_addr();
        let donor2 = "donor2".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        let funds1 = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(300),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor1, &funds1)));
        contract.donate(exec_ctx, 1).unwrap();

        let funds2 = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(400),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor2, &funds2)));
        contract.donate(exec_ctx, 1).unwrap();

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert_eq!(project.total_donated, Uint128::new(700));
        assert!(project.status == ProjectStatus::Fundraising);
    }

    #[test]
    fn donate_exact_difference() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor1 = "donor1".into_addr();
        let donor2 = "donor2".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        let funds1 = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(600),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor1, &funds1)));
        contract.donate(exec_ctx, 1).unwrap();

        let funds2 = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(400),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor2, &funds2)));
        let res = contract.donate(exec_ctx, 1).unwrap();

        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "action" && a.value == "donate")
        );

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert_eq!(project.total_donated, project.goal);
        assert!(project.status == ProjectStatus::Funded);
    }

    #[test]
    fn donate_fails_after_goal_met() {
        let admin = "admin".into_addr();
        let donor = "donor".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let create_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(create_ctx, Uint128::from(1000u128), valid_json)
            .unwrap();

        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        let donation = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::from(1000u128),
        }];

        let donate_ctx =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &donation)));
        contract.donate(donate_ctx, 1).unwrap();

        // Second donation after goal met
        let donate_ctx_2 =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &donation)));
        let err = contract.donate(donate_ctx_2, 1).unwrap_err();

        assert_eq!(err.to_string(), "Generic error: Project is already funded");
    }

    #[test]
    fn donate_after_completion_fails() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Donor donates full amount
        let donation = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &donation)));
        contract.donate(exec_ctx, 1).unwrap();

        // Admin unlocks project
        let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.unlock(unlock_ctx, 1).unwrap();

        // Owner disburses project
        let disburse_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.disburse(disburse_ctx, 1).unwrap();

        // Donor tries to donate again after completion
        let extra_funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(100),
        }];
        let exec_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&donor, &extra_funds),
        ));
        let err = contract.donate(exec_ctx, 1).unwrap_err();

        assert_eq!(
            err.to_string(),
            "Generic error: Project is already disbursed and completed"
        );

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert_eq!(project.status, ProjectStatus::Completed);
        assert_eq!(project.total_donated, Uint128::new(1000));
    }

    #[test]
    fn refund_success() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        // Instantiate contract
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Save a cancelled project
        let project = Project {
            id: 1,
            owner: owner.clone(),
            goal: Uint128::from(1000u128),
            total_donated: Uint128::from(500u128),
            status: ProjectStatus::Cancelled,
            data_str: valid_json_str().to_string(),
        };
        PROJECTS.save(deps.as_mut().storage, 1, &project).unwrap();

        // Save donation from donor
        DONATIONS
            .save(
                deps.as_mut().storage,
                (1, &donor.clone()),
                &Uint128::from(300u128),
            )
            .unwrap();

        // Refund called by donor
        let refund_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &[])));
        let res = contract.refund(refund_ctx, 1).unwrap();

        let attrs: std::collections::HashMap<_, _> = res
            .attributes
            .iter()
            .map(|attr| (attr.key.as_str(), attr.value.as_str()))
            .collect();

        assert_eq!(attrs.get("action"), Some(&"refund"));
        assert_eq!(attrs.get("project_id"), Some(&"1"));
        assert_eq!(attrs.get("refunded_to"), Some(&donor.as_str()));
        assert_eq!(attrs.get("amount"), Some(&"300"));
        assert_eq!(attrs.get("status"), Some(&"Cancelled"));

        // Donation entry removed after refund
        let donation_after = DONATIONS
            .may_load(deps.as_ref().storage, (1, &donor.clone()))
            .unwrap();
        assert!(donation_after.is_none());
    }

    #[test]
    fn refund_fail_not_cancelled() {
        let admin = "admin".into_addr();
        let donor = "donor".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        // Instantiate contract
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Save a project NOT cancelled
        let project = Project {
            id: 1,
            owner: admin.clone(),
            goal: Uint128::from(1000u128),
            total_donated: Uint128::from(300u128),
            status: ProjectStatus::Fundraising,
            data_str: valid_json_str().to_string(),
        };
        PROJECTS.save(deps.as_mut().storage, 1, &project).unwrap();

        // Save donation from donor
        DONATIONS
            .save(
                deps.as_mut().storage,
                (1, &donor.clone()),
                &Uint128::from(300u128),
            )
            .unwrap();

        let refund_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &[])));
        let err = contract.refund(refund_ctx, 1).unwrap_err();

        assert!(
            err.to_string()
                .contains(&ContractError::NotRefundable.to_string())
        );
    }

    #[test]
    fn refund_fail_no_donation() {
        let admin = "admin".into_addr();
        let donor = "donor".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        // Instantiate contract
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Save a cancelled project
        let project = Project {
            id: 1,
            owner: admin.clone(),
            goal: Uint128::from(1000u128),
            total_donated: Uint128::from(0u128),
            status: ProjectStatus::Cancelled,
            data_str: valid_json_str().to_string(),
        };
        PROJECTS.save(deps.as_mut().storage, 1, &project).unwrap();

        // No donation saved for donor here
        let refund_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &[])));
        let err = contract.refund(refund_ctx, 1).unwrap_err();

        assert!(
            err.to_string()
                .contains(&ContractError::NoRefundAvailable.to_string())
        );
    }

    #[test]
    fn unlock_success() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Validate
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Donate full amount
        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let donate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(donate_ctx, 1).unwrap();

        let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let res = contract.unlock(unlock_ctx, 1).unwrap();

        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "action" && a.value == "unlock")
        );

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert_eq!(project.status, ProjectStatus::Disbursable);
    }

    #[test]
    fn unlock_fail_not_admin() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Owner tries to unlock instead of admin
        let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let err = contract.unlock(unlock_ctx, 1).unwrap_err();

        assert_eq!(err.to_string(), "Generic error: Unauthorized");
    }

    #[test]
    fn unlock_fail_goal_not_met() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Partial donation
        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(500),
        }];
        let donate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(donate_ctx, 1).unwrap();

        // Try unlock
        let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.unlock(unlock_ctx, 1).unwrap_err();

        assert_eq!(err.to_string(), "Generic error: Goal not reached yet");
    }

    #[test]
    fn unlock_fail_already_disbursable() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Validate and fund
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let donate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(donate_ctx, 1).unwrap();

        // First unlock
        let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.unlock(unlock_ctx, 1).unwrap();

        // Second unlock (invalid)
        let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.unlock(unlock_ctx, 1).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Generic error: Project is already marked as disbursable"
        );
    }

    #[test]
    fn unlock_fail_already_completed() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Validate and donate full amount
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let donate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(donate_ctx, 1).unwrap();

        // Unlock and disburse
        let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.unlock(unlock_ctx, 1).unwrap();

        let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.disburse(unlock_ctx, 1).unwrap();

        // Try to unlock again
        let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.unlock(unlock_ctx, 1).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Generic error: Project is already disbursed and completed"
        );
    }

    #[test]
    fn disburse_exact_goal_not_funded() {
        let admin = "admin".into_addr();
        let contract = WaterWellInitiativeContract::new();

        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let project1 = Project {
            id: 1,
            owner: admin.clone(),
            goal: Uint128::from(1000u128),
            total_donated: Uint128::from(1000u128),
            status: ProjectStatus::Fundraising,
            data_str: valid_json_str().to_string(),
        };

        PROJECTS.save(deps.as_mut().storage, 1, &project1).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.disburse(exec_ctx, 1).unwrap_err();

        // Goal is reached but project flagged as not funded, this case should not happen but is tested, both goal and funded should be met before disburse
        assert_eq!(
            err.to_string(),
            "Generic error: Project is not yet ready for disbursal"
        );

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert!(project.status == ProjectStatus::Fundraising);
    }

    #[test]
    fn disburse_fails_before_unlock() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        contract
            .instantiate(
                InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                Some(DEFAULT_DENOM.to_string()),
            )
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[]))),
                Uint128::from(500u128),
                valid_json,
            )
            .unwrap();

        contract
            .validate(
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[]))),
                1,
            )
            .unwrap();

        contract
            .donate(
                ExecCtx::from((
                    deps.as_mut(),
                    mock_env(),
                    message_info(
                        &owner,
                        &[Coin {
                            denom: DEFAULT_DENOM.to_string(),
                            amount: Uint128::from(500u128),
                        }],
                    ),
                )),
                1,
            )
            .unwrap();

        // Disburse before unlock
        let disburse_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let err = contract.disburse(disburse_ctx, 1).unwrap_err();

        assert_eq!(
            err.to_string(),
            "Generic error: Project is not yet ready for disbursal"
        );
    }

    #[test]
    fn disburse_admin_success() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Donor donates 1000 uatom to reach goal
        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(exec_ctx, 1).unwrap();

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert!(project.status == ProjectStatus::Funded);

        // Admin unlocks funds
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.unlock(exec_ctx, 1).unwrap();

        // Admin disburses funds
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let res = contract.disburse(exec_ctx, 1).unwrap();

        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "action" && a.value == "disburse")
        );
        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "project_id" && a.value == "1")
        );

        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "sender" && a.value == admin.to_string())
        );
        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "recipient" && a.value == owner.to_string())
        );
        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "amount" && a.value == "1000")
        );

        // Assert message send exists
        assert_eq!(res.messages.len(), 1);

        // Check project marked funded
        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert!(project.status == ProjectStatus::Completed);
    }

    #[test]
    fn disburse_owner_success() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Donor donates 1000 uatom to reach goal
        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(exec_ctx, 1).unwrap();

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert!(project.status == ProjectStatus::Funded);

        // Admin unlocks funds
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.unlock(exec_ctx, 1).unwrap();

        // Owner disburses funds
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let res = contract.disburse(exec_ctx, 1).unwrap();

        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "action" && a.value == "disburse")
        );
        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "project_id" && a.value == "1")
        );

        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "sender" && a.value == owner.to_string())
        );
        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "recipient" && a.value == owner.to_string())
        );
        assert!(
            res.attributes
                .iter()
                .any(|a| a.key == "amount" && a.value == "1000")
        );

        // Assert message send exists
        assert_eq!(res.messages.len(), 1);

        // Check project marked funded
        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert!(project.status == ProjectStatus::Completed);
    }

    #[test]
    fn disburse_fail_not_validated() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Owner tries to disburse before admin validation
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.disburse(exec_ctx, 1).unwrap_err();

        assert_eq!(
            err.to_string(),
            "Generic error: Project is not yet ready for disbursal"
        );

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert!(project.status == ProjectStatus::Proposed);
    }

    #[test]
    fn disburse_fail_goal_not_reached() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Donor donates only 500 uatom
        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(500),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(exec_ctx, 1).unwrap();

        // Admin tries to unlock funds
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.unlock(exec_ctx, 1).unwrap_err();

        assert_eq!(err.to_string(), "Generic error: Goal not reached yet");

        // Owner tries to disburse before goal reached and unlock
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.disburse(exec_ctx, 1).unwrap_err();

        assert_eq!(
            err.to_string(),
            "Generic error: Project is not yet ready for disbursal"
        );

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert!(project.status == ProjectStatus::Fundraising);
    }

    #[test]
    fn disburse_fail_already_completed() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Donor donates 1000 uatom
        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(exec_ctx, 1).unwrap();

        // Admin unlocks funds
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.unlock(exec_ctx, 1).unwrap();

        // Owner disburses once
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.disburse(exec_ctx, 1).unwrap();

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert!(project.status == ProjectStatus::Completed);

        // Owner tries to disburse again
        let exec_ctx2 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.disburse(exec_ctx2, 1).unwrap_err();

        assert_eq!(
            err.to_string(),
            "Generic error: Project is already disbursed and completed"
        );

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert!(project.status == ProjectStatus::Completed);
    }

    #[test]
    fn disburse_fail_already_completed_from_state() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();
        let contract = WaterWellInitiativeContract::new();

        // Instantiate contract
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Create a project with Funded status
        let project_id = 1;
        let project = Project {
            id: project_id,
            owner: owner.clone(),
            goal: Uint128::from(1000u128),
            total_donated: Uint128::from(1000u128),
            status: ProjectStatus::Disbursable,
            data_str: valid_json_str().to_string(),
        };
        PROJECTS
            .save(deps.as_mut().storage, project_id, &project)
            .unwrap();

        // Disburse first time: should succeed
        let disburse_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let res = contract.disburse(disburse_ctx, project_id);
        assert!(res.is_ok());

        // Check that project status is now Completed
        let saved_project = PROJECTS.load(deps.as_ref().storage, project_id).unwrap();
        assert_eq!(saved_project.status, ProjectStatus::Completed);

        // Try to disburse again: should fail with AlreadyCompleted error
        let disburse_ctx_again =
            ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let err = contract
            .disburse(disburse_ctx_again, project_id)
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("Project is already disbursed and completed"),
            "Expected AlreadyCompleted error, got: {err}"
        );
    }

    #[test]
    fn disburse_fail_not_owner() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let stranger = "stranger".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellInitiativeContract::new();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000), valid_json)
            .unwrap();

        // Admin validates project
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.validate(validate_ctx, 1).unwrap();

        // Donor donates 1000 uatom to reach goal
        let funds = vec![Coin {
            denom: DEFAULT_DENOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(exec_ctx, 1).unwrap();

        // Admin unlocks funds
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract.unlock(exec_ctx, 1).unwrap();

        // Non-owner tries to disburse
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&stranger, &[])));
        let err = contract.disburse(exec_ctx, 1).unwrap_err();

        assert!(
            err.to_string().contains("Unauthorized"),
            "Expected unauthorized error, got: {err}"
        );

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert!(project.status == ProjectStatus::Disbursable);
    }

    #[test]
    fn list_projects_empty() {
        let contract = WaterWellInitiativeContract::new();
        let deps = mock_dependencies();
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        let projects = contract.list_projects(query_ctx, None, None).unwrap();
        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn list_projects_multiple() {
        let admin = "admin".into_addr();
        let contract = WaterWellInitiativeContract::new();

        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        let project1 = Project {
            id: 1,
            owner: admin.clone(),
            goal: Uint128::from(1000u128),
            total_donated: Uint128::from(100u128),
            status: ProjectStatus::default(),
            data_str: valid_json_str().to_string(),
        };
        let project2 = Project {
            id: 2,
            owner: admin.clone(),
            goal: Uint128::from(2000u128),
            total_donated: Uint128::from(500u128),
            status: ProjectStatus::default(),
            data_str: valid_json_str().to_string(),
        };
        PROJECTS.save(deps.as_mut().storage, 1, &project1).unwrap();
        PROJECTS.save(deps.as_mut().storage, 2, &project2).unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let projects = contract.list_projects(query_ctx, None, None).unwrap();

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].id, 1);
        assert_eq!(projects[1].id, 2);
    }

    #[test]
    fn list_projects_pagination() {
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();

        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Create 5 projects starting with ID 1
        for i in 0u64..5 {
            let id = i + 1; // IDs: 1 to 5
            let project = Project {
                id,
                owner: owner.clone(),
                goal: Uint128::from(1000u128 + i as u128 * 100),
                total_donated: Uint128::zero(),
                status: ProjectStatus::default(),
                data_str: valid_json_str().to_string(),
            };
            PROJECTS.save(deps.as_mut().storage, id, &project).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Page 1: fetch first 2 (IDs 1, 2)
        let first_page = contract.list_projects(query_ctx, Some(2), None).unwrap();
        assert_eq!(first_page.len(), 2);
        assert_eq!(first_page[0].id, 1);
        assert_eq!(first_page[1].id, 2);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Page 2: start after ID 2
        let second_page = contract.list_projects(query_ctx, Some(2), Some(2)).unwrap();
        assert_eq!(second_page.len(), 2);
        assert_eq!(second_page[0].id, 3);
        assert_eq!(second_page[1].id, 4);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Page 3: start after ID 4
        let third_page = contract.list_projects(query_ctx, Some(2), Some(4)).unwrap();
        assert_eq!(third_page.len(), 1);
        assert_eq!(third_page[0].id, 5);
    }

    #[test]
    fn list_projects_default_limit() {
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Create 15 projects
        for i in 1u64..=15 {
            let project = Project {
                id: i,
                owner: owner.clone(),
                goal: Uint128::from(1000u128),
                total_donated: Uint128::zero(),
                status: ProjectStatus::default(),
                data_str: valid_json_str().to_string(),
            };
            PROJECTS.save(deps.as_mut().storage, i, &project).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // No limit provided — should default to 10
        let result = contract.list_projects(query_ctx, None, None).unwrap();
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn list_projects_start_after_last_id_returns_empty() {
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Create 3 projects
        for i in 1u64..=3 {
            let project = Project {
                id: i,
                owner: owner.clone(),
                goal: Uint128::from(1000u128),
                total_donated: Uint128::zero(),
                status: ProjectStatus::default(),
                data_str: valid_json_str().to_string(),
            };
            PROJECTS.save(deps.as_mut().storage, i, &project).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // start_after = 3 (last project), expect empty result
        let result = contract
            .list_projects(query_ctx, Some(10), Some(3))
            .unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn list_projects_limit_caps_at_30() {
        let owner = "owner".into_addr();
        let contract = WaterWellInitiativeContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Create 50 projects
        for i in 1u64..=50 {
            let project = Project {
                id: i,
                owner: owner.clone(),
                goal: Uint128::from(1000u128),
                total_donated: Uint128::zero(),
                status: ProjectStatus::default(),
                data_str: valid_json_str().to_string(),
            };
            PROJECTS.save(deps.as_mut().storage, i, &project).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Pass a huge limit — should still only get 30
        let result = contract.list_projects(query_ctx, Some(999), None).unwrap();
        assert_eq!(result.len(), 30);
    }

    #[test]
    fn get_project_status_counts_reflects_all_statuses() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();
        let contract = WaterWellInitiativeContract::new();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Create 8 projects, one by one, so IDs go from 1 to 8.
        for _ in 0..8 {
            let exec_ctx = ExecCtx::from((
                deps.as_mut(),
                mock_env(),
                message_info(
                    &owner,
                    &[Coin {
                        denom: DEFAULT_DENOM.to_string(),
                        amount: Uint128::zero(),
                    }],
                ),
            ));
            let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

            contract
                .create_project(exec_ctx, Uint128::from(1000u128), valid_json.clone())
                .unwrap();
        }

        // IDs 1 and 2 stay Proposed (no changes)

        // IDs 3 and 4 => Fundraising by validating
        for id in 3..=4 {
            let validate_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
            contract.validate(validate_ctx, id).unwrap();
        }

        // IDs 5 and 6 => Funded by validating and donating some amount < goal
        for id in 5..=6 {
            let validate_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
            contract.validate(validate_ctx, id).unwrap();

            let donate_ctx = ExecCtx::from((
                deps.as_mut(),
                mock_env(),
                message_info(
                    &owner,
                    &[Coin {
                        denom: DEFAULT_DENOM.to_string(),
                        amount: Uint128::from(1000u128), // full goal
                    }],
                ),
            ));
            contract.donate(donate_ctx, id).unwrap();
        }

        // ID 7 => Disbursable by validating, donating, and unlocking
        {
            let id = 7;

            let validate_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
            contract.validate(validate_ctx, id).unwrap();

            let donate_ctx = ExecCtx::from((
                deps.as_mut(),
                mock_env(),
                message_info(
                    &owner,
                    &[Coin {
                        denom: DEFAULT_DENOM.to_string(),
                        amount: Uint128::from(1000u128),
                    }],
                ),
            ));
            contract.donate(donate_ctx, id).unwrap();

            let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
            contract.unlock(unlock_ctx, id).unwrap();
        }

        // ID 8 => Completed by validating, donating, unlocking, and disbursing
        {
            let id = 8;

            let validate_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
            contract.validate(validate_ctx, id).unwrap();

            let donate_ctx = ExecCtx::from((
                deps.as_mut(),
                mock_env(),
                message_info(
                    &owner,
                    &[Coin {
                        denom: DEFAULT_DENOM.to_string(),
                        amount: Uint128::from(1000u128),
                    }],
                ),
            ));
            contract.donate(donate_ctx, id).unwrap();

            let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
            contract.unlock(unlock_ctx, id).unwrap();

            let disburse_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
            contract.disburse(disburse_ctx, id).unwrap();
        }

        // Query status counts
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let counts = contract.get_project_status_counts(query_ctx).unwrap();

        assert_eq!(counts.get(&ProjectStatus::Proposed.to_string()), Some(&2)); // IDs 1,2
        assert_eq!(
            counts.get(&ProjectStatus::Fundraising.to_string()),
            Some(&2)
        ); // IDs 3,4
        assert_eq!(counts.get(&ProjectStatus::Funded.to_string()), Some(&2)); // IDs 5,6
        assert_eq!(
            counts.get(&ProjectStatus::Disbursable.to_string()),
            Some(&1)
        ); // ID 7
        assert_eq!(counts.get(&ProjectStatus::Completed.to_string()), Some(&1)); // ID 8
        assert_eq!(counts.get(&ProjectStatus::Cancelled.to_string()), Some(&0));
    }

    #[test]
    fn get_projects_by_status() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();
        let contract = WaterWellInitiativeContract::new();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Create 3 projects, all start as Proposed
        for _ in 0..3 {
            let exec_ctx = ExecCtx::from((
                deps.as_mut(),
                mock_env(),
                message_info(
                    &owner,
                    &[Coin {
                        denom: DEFAULT_DENOM.to_string(),
                        amount: Uint128::zero(),
                    }],
                ),
            ));
            let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

            contract
                .create_project(exec_ctx, Uint128::from(1000u128), valid_json.clone())
                .unwrap();
        }

        // Validate 2 projects to move them to Fundraising
        for project_id in 1..=2 {
            let validate_ctx =
                ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
            contract.validate(validate_ctx, project_id).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Query Proposed projects
        let proposed_projects = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Proposed, None, None)
            .unwrap();
        assert_eq!(proposed_projects.len(), 1);
        assert_eq!(proposed_projects[0].status, ProjectStatus::Proposed);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Query Fundraising projects
        let fundraising_projects = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Fundraising, None, None)
            .unwrap();
        assert_eq!(fundraising_projects.len(), 2);
        for project in fundraising_projects.iter() {
            assert_eq!(project.status, ProjectStatus::Fundraising);
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Query Funded projects (none yet)
        let funded_projects = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Funded, None, None)
            .unwrap();
        assert_eq!(funded_projects.len(), 0);
    }

    #[test]
    fn get_projects_by_status_pagination_with_loop() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();
        let contract = WaterWellInitiativeContract::new();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert 50 Proposed projects directly
        for i in 1u64..=50 {
            let project = Project {
                id: i,
                owner: owner.clone(),
                goal: Uint128::from(1000u128),
                total_donated: Uint128::zero(),
                status: ProjectStatus::default(), // Proposed
                data_str: valid_json_str().to_string(),
            };
            PROJECTS.save(deps.as_mut().storage, i, &project).unwrap();
        }

        // Query first page (10 results)
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let page_1 = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Proposed, Some(10), None)
            .unwrap();

        assert_eq!(page_1.len(), 10);
        assert_eq!(page_1[0].id, 1);
        assert_eq!(page_1[9].id, 10);

        // Query second page (next 10 results)
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let page_2 = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Proposed, Some(10), Some(10))
            .unwrap();

        assert_eq!(page_2.len(), 10);
        assert_eq!(page_2[0].id, 11);
        assert_eq!(page_2[9].id, 20);

        // Final page (start_after = 49)
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let final_page = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Proposed, Some(10), Some(49))
            .unwrap();

        assert_eq!(final_page.len(), 1);
        assert_eq!(final_page[0].id, 50);
    }

    #[test]
    fn get_projects_by_status_pagination_skips_gaps_and_filters_by_status() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();
        let contract = WaterWellInitiativeContract::new();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for i in 1..=100 {
            let status = if i % 7 == 0 {
                ProjectStatus::Proposed
            } else {
                ProjectStatus::Cancelled
            };

            let project = Project {
                id: i,
                owner: owner.clone(),
                goal: Uint128::from(1000u128),
                total_donated: Uint128::zero(),
                status,
                data_str: valid_json_str().to_string(),
            };
            PROJECTS.save(deps.as_mut().storage, i, &project).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let page = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Proposed, Some(10), Some(20))
            .unwrap();

        assert_eq!(page.len(), 10);
        assert_eq!(page[0].id, 21);
        assert_eq!(page[1].id, 28);
    }

    #[test]
    fn get_projects_by_status_pagination_across_pages() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();
        let contract = WaterWellInitiativeContract::new();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert alternating Proposed and Cancelled projects (IDs 1..=9)
        for i in 1..=9 {
            let status = if i % 2 == 0 {
                ProjectStatus::Cancelled
            } else {
                ProjectStatus::Proposed
            };

            let project = Project {
                id: i,
                owner: owner.clone(),
                goal: Uint128::from(1000u128),
                total_donated: Uint128::zero(),
                status,
                data_str: valid_json_str().to_string(),
            };
            PROJECTS.save(deps.as_mut().storage, i, &project).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // PAGE 1 — Should return IDs 1 and 3 (first 2 Proposed projects)
        let page1 = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Proposed, Some(2), None)
            .unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].id, 1);
        assert_eq!(page1[1].id, 3);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        // PAGE 2 — Start after 3, should return ID 5 and 7
        let page2 = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Proposed, Some(2), Some(3))
            .unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].id, 5);
        assert_eq!(page2[1].id, 7);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        // PAGE 3 — Start after 7, should return ID 9 only
        let page3 = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Proposed, Some(2), Some(7))
            .unwrap();
        assert_eq!(page3.len(), 1);
        assert_eq!(page3[0].id, 9);
    }

    #[test]
    fn get_projects_by_status_default_limit_none() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();
        let contract = WaterWellInitiativeContract::new();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        for i in 1..=30 {
            let status = if i <= 15 {
                ProjectStatus::Proposed
            } else {
                ProjectStatus::Cancelled
            };

            let project = Project {
                id: i,
                owner: owner.clone(),
                goal: Uint128::from(1000u128),
                total_donated: Uint128::zero(),
                status,
                data_str: valid_json_str().to_string(),
            };
            PROJECTS.save(deps.as_mut().storage, i, &project).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let result = contract
            .get_projects_by_status(query_ctx, ProjectStatus::Proposed, None, None)
            .unwrap();

        assert_eq!(result.len(), 10);
        assert_eq!(result[0].id, 1);
        assert_eq!(result[9].id, 10);
    }

    #[test]
    fn methods_fail_on_cancelled_project() {
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();
        let contract = WaterWellInitiativeContract::new();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        contract
            .instantiate(ctx, Some(DEFAULT_DENOM.to_string()))
            .unwrap();

        // Insert one Cancelled project with id 1 manually
        let project = Project {
            id: 1,
            owner: owner.clone(),
            goal: Uint128::from(1000u128),
            total_donated: Uint128::zero(),
            status: ProjectStatus::Cancelled,
            data_str: valid_json_str().to_string(),
        };
        PROJECTS.save(deps.as_mut().storage, 1, &project).unwrap();

        // Validate - should error with cancelled
        let validate_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.validate(validate_ctx, 1).unwrap_err();
        assert!(
            err.to_string().contains("Project is cancelled"),
            "Expected cancelled error, got: {err}"
        );

        // Donate - should error with cancelled
        let donate_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(
                &owner,
                &[Coin {
                    denom: DEFAULT_DENOM.to_string(),
                    amount: Uint128::from(1000u128),
                }],
            ),
        ));
        let err = contract.donate(donate_ctx, 1).unwrap_err();
        assert!(
            err.to_string().contains("Project is cancelled"),
            "Expected cancelled error, got: {err}"
        );

        // Unlock - should error with cancelled
        let unlock_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&admin, &[])));
        let err = contract.unlock(unlock_ctx, 1).unwrap_err();
        assert!(
            err.to_string().contains("Project is cancelled"),
            "Expected cancelled error, got: {err}"
        );

        // Disburse - should error with cancelled
        let disburse_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let err = contract.disburse(disburse_ctx, 1).unwrap_err();
        assert!(
            err.to_string().contains("Project is cancelled"),
            "Expected cancelled error, got: {err}"
        );
    }
}
