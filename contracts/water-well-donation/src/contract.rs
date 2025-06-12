use cosmwasm_schema::cw_serde;
use cw_storage_plus::{Bound, Item, Map};
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_std::{Addr, BankMsg, Coin, Order, Response, StdResult, Uint128};
use sylvia::entry_points;

use crate::errors::ContractError;

#[cw_serde]
pub struct Project {
    pub id: u64,
    pub owner: Addr,
    pub goal: Uint128,
    pub total_donated: Uint128,
    pub funded: bool,
}

const OWNER: Item<Addr> = Item::new("owner");
const PROJECTS: Map<u64, Project> = Map::new("projects");
const NEXT_ID: Item<u64> = Item::new("next_id");
pub const UATOM: &str = "ustake";

pub struct WaterWellDonationContract;

#[cfg_attr(not(feature = "library"), entry_points)]
#[contract]
#[sv::error(ContractError)]
impl WaterWellDonationContract {
    pub const fn new() -> Self {
        Self
    }

    #[sv::msg(instantiate)]
    fn instantiate(&self, ctx: InstantiateCtx) -> StdResult<Response> {
        OWNER.save(ctx.deps.storage, &ctx.info.sender)?;
        NEXT_ID.save(ctx.deps.storage, &1)?;
        Ok(Response::new().add_attribute("method", "instantiate"))
    }

    #[sv::msg(exec)]
    fn create_project(&self, ctx: ExecCtx, goal: Uint128) -> StdResult<Response> {
        let owner = OWNER.load(ctx.deps.storage)?;
        if ctx.info.sender != owner {
            return Err(ContractError::Unauthorized.into());
        }
        if goal.is_zero() {
            return Err(ContractError::ZeroGoal.into());
        }

        let id = NEXT_ID.load(ctx.deps.storage)?;
        let project = Project {
            id,
            owner: ctx.info.sender.clone(),
            goal,
            total_donated: Uint128::zero(),
            funded: false,
        };

        PROJECTS.save(ctx.deps.storage, id, &project)?;
        NEXT_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "create_project")
            .add_attribute("project_id", id.to_string()))
    }

    #[sv::msg(exec)]
    fn donate(&self, ctx: ExecCtx, project_id: u64) -> StdResult<Response> {
        let mut project = PROJECTS.load(ctx.deps.storage, project_id)?;

        let donation = ctx
            .info
            .funds
            .iter()
            .find(|c| c.denom == UATOM)
            .map(|c| c.amount)
            .unwrap_or_default();

        if donation.is_zero() {
            return Err(ContractError::NoDonation.into());
        }

        if project.total_donated + donation > project.goal {
            return Err(ContractError::ExceedGoal.into());
        }

        project.total_donated += donation;
        PROJECTS.save(ctx.deps.storage, project_id, &project)?;

        Ok(Response::new()
            .add_attribute("action", "donate")
            .add_attribute("project_id", project_id.to_string())
            .add_attribute("donor", ctx.info.sender.to_string())
            .add_attribute("amount", donation.to_string()))
    }

    #[sv::msg(exec)]
    fn disburse(&self, ctx: ExecCtx, project_id: u64) -> StdResult<Response> {
        let mut project = PROJECTS.load(ctx.deps.storage, project_id)?;

        // Check owner authorization
        if ctx.info.sender != project.owner {
            return Err(ContractError::Unauthorized.into());
        }

        if project.total_donated < project.goal {
            return Err(ContractError::GoalNotMet.into());
        }

        if project.funded {
            return Err(ContractError::AlreadyDisbursed.into());
        }

        project.funded = true;
        PROJECTS.save(ctx.deps.storage, project_id, &project)?;

        let send_msg = BankMsg::Send {
            to_address: project.owner.to_string(),
            amount: vec![Coin {
                denom: UATOM.to_string(),
                amount: project.total_donated,
            }],
        };

        Ok(Response::new()
            .add_message(send_msg)
            .add_attribute("action", "disburse")
            .add_attribute("project_id", project_id.to_string())
            .add_attribute("recipient", project.owner.to_string())
            .add_attribute("amount", project.total_donated.to_string()))
    }

    #[sv::msg(query)]
    fn get_project(&self, ctx: QueryCtx, project_id: u64) -> StdResult<Project> {
        PROJECTS.load(ctx.deps.storage, project_id)
    }

    #[sv::msg(query)]
    fn list_projects(
        &self,
        ctx: QueryCtx,
        start_after: Option<u64>,
        limit: Option<u32>,
    ) -> StdResult<Vec<Project>> {
        let limit = limit.unwrap_or(10).min(30) as usize;

        let start = start_after.map(Bound::exclusive);

        PROJECTS
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| item.map(|(_, p)| p))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sylvia::cw_multi_test::IntoAddr;
    use sylvia::cw_std::StdError;
    use sylvia::cw_std::testing::{message_info, mock_dependencies, mock_env};

    #[test]
    fn init() {
        let owner = "owner".into_addr();
        let contract = WaterWellDonationContract::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(ctx).unwrap();

        let stored_owner = OWNER.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_owner, owner);

        let stored_next_id = NEXT_ID.load(deps.as_ref().storage).unwrap();
        assert_eq!(stored_next_id, 1);
    }

    #[test]
    fn query_nonexistent_project() {
        let owner = "owner".into_addr();
        let contract = WaterWellDonationContract::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(ctx).unwrap();

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
        let owner = "owner".into_addr();
        let contract = WaterWellDonationContract::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        let goal = Uint128::from(1000u128);
        let exec_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(
                &owner,
                &[Coin {
                    denom: UATOM.to_string(),
                    amount: Uint128::zero(),
                }],
            ),
        ));
        contract.create_project(exec_ctx, goal).unwrap();

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let res = contract.get_project(query_ctx, 1).unwrap();

        assert_eq!(res.id, 1);
        assert_eq!(res.owner, owner);
        assert_eq!(res.goal, goal);
        assert_eq!(res.total_donated, Uint128::zero());
        assert!(!res.funded);
    }

    #[test]
    fn create_project_fail_zero_goal() {
        let owner = "owner".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellDonationContract::new();
        let instantiate_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(instantiate_ctx).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));

        let err = contract
            .create_project(exec_ctx, Uint128::zero())
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Generic error: Goal must be greater than zero"
        );
    }

    #[test]
    fn create_project_unauthorized() {
        let owner = "owner".into_addr();
        let not_owner = "not_owner".into_addr();
        let contract = WaterWellDonationContract::new();
        let mut deps = mock_dependencies();

        let inst_ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(inst_ctx).unwrap();

        // Attempt to create project with a non-owner sender
        let exec_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(
                &not_owner,
                &[Coin {
                    denom: UATOM.to_string(),
                    amount: Uint128::zero(),
                }],
            ),
        ));

        let goal = Uint128::from(1000u128);
        let err = contract.create_project(exec_ctx, goal).unwrap_err();
        assert!(
            err.to_string().contains("Unauthorized"),
            "Expected unauthorized error, got: {err}"
        );
    }

    #[test]
    fn donate_success() {
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellDonationContract::new();
        let instantiate_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(instantiate_ctx).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000))
            .unwrap();

        // Donor sends 500 uatom
        let funds = vec![Coin {
            denom: UATOM.to_string(),
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
        assert!(!project.funded);
    }

    #[test]
    fn donate_no_funds_fails() {
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellDonationContract::new();
        let instantiate_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(instantiate_ctx).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000))
            .unwrap();

        // Donor sends no funds
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &[])));

        let err = contract.donate(exec_ctx, 1).unwrap_err();
        assert_eq!(err.to_string(), "Generic error: No valid donation");
    }

    #[test]
    fn donate_exceed_goal_fails() {
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellDonationContract::new();
        let instantiate_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(instantiate_ctx).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000))
            .unwrap();

        // Donor sends 1200 uatom (exceeds goal)
        let funds = vec![Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(1200),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));

        let err = contract.donate(exec_ctx, 1).unwrap_err();
        assert_eq!(err.to_string(), "Generic error: Donation exceeds goal");
    }

    #[test]
    fn donate_multiple_partial() {
        let owner = "owner".into_addr();
        let donor1 = "donor1".into_addr();
        let donor2 = "donor2".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellDonationContract::new();
        let instantiate_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(instantiate_ctx).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000))
            .unwrap();

        let funds1 = vec![Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(300),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor1, &funds1)));
        contract.donate(exec_ctx, 1).unwrap();

        let funds2 = vec![Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(400),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor2, &funds2)));
        contract.donate(exec_ctx, 1).unwrap();

        let project = PROJECTS.load(&deps.storage, 1).unwrap();
        assert_eq!(project.total_donated, Uint128::new(700));
    }

    #[test]
    fn donate_exact_difference() {
        let owner = "owner".into_addr();
        let donor1 = "donor1".into_addr();
        let donor2 = "donor2".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellDonationContract::new();
        let instantiate_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(instantiate_ctx).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000))
            .unwrap();

        let funds1 = vec![Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(600),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor1, &funds1)));
        contract.donate(exec_ctx, 1).unwrap();

        let funds2 = vec![Coin {
            denom: UATOM.to_string(),
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
    }

    #[test]
    fn disburse_success() {
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellDonationContract::new();
        let instantiate_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(instantiate_ctx).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000))
            .unwrap();

        // Donor donates 1000 uatom to reach goal
        let funds = vec![Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(exec_ctx, 1).unwrap();

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
        assert!(project.funded);
    }

    #[test]
    fn disburse_fail_goal_not_reached() {
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellDonationContract::new();
        let instantiate_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(instantiate_ctx).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000))
            .unwrap();

        // Donor donates only 500 uatom
        let funds = vec![Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(500),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(exec_ctx, 1).unwrap();

        // Owner tries to disburse before goal reached
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let err = contract.disburse(exec_ctx, 1).unwrap_err();

        assert_eq!(err.to_string(), "Generic error: Goal not reached yet");
    }

    #[test]
    fn disburse_fail_already_funded() {
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellDonationContract::new();
        let instantiate_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(instantiate_ctx).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000))
            .unwrap();

        // Donor donates 1000 uatom
        let funds = vec![Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(exec_ctx, 1).unwrap();

        // Owner disburses once
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.disburse(exec_ctx, 1).unwrap();

        // Owner tries to disburse again
        let exec_ctx2 = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        let err = contract.disburse(exec_ctx2, 1).unwrap_err();

        assert_eq!(err.to_string(), "Generic error: Already disbursed");
    }

    #[test]
    fn disburse_fail_not_owner() {
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();
        let stranger = "stranger".into_addr();
        let mut deps = mock_dependencies();

        let contract = WaterWellDonationContract::new();
        let instantiate_ctx =
            InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(instantiate_ctx).unwrap();

        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract
            .create_project(exec_ctx, Uint128::new(1000))
            .unwrap();

        // Donor donates 1000 uatom to reach goal
        let funds = vec![Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(1000),
        }];
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&donor, &funds)));
        contract.donate(exec_ctx, 1).unwrap();

        // Non-owner tries to disburse
        let exec_ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&stranger, &[])));
        let err = contract.disburse(exec_ctx, 1).unwrap_err();

        assert!(
            err.to_string().contains("Unauthorized"),
            "Expected unauthorized error, got: {err}"
        );
    }

    #[test]
    fn test_list_projects_empty() {
        let contract = WaterWellDonationContract::new();
        let deps = mock_dependencies();
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        let projects = contract.list_projects(query_ctx, None, None).unwrap();
        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn test_list_projects_multiple() {
        let owner = "owner".into_addr();
        let contract = WaterWellDonationContract::new();

        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(ctx).unwrap();

        let project1 = Project {
            id: 1,
            owner: owner.clone(),
            goal: Uint128::from(1000u128),
            total_donated: Uint128::from(100u128),
            funded: false,
        };
        let project2 = Project {
            id: 2,
            owner: owner.clone(),
            goal: Uint128::from(2000u128),
            total_donated: Uint128::from(500u128),
            funded: false,
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
    fn test_list_projects_pagination() {
        let owner = "carol".into_addr();
        let contract = WaterWellDonationContract::new();

        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&owner, &[])));
        contract.instantiate(ctx).unwrap();

        // Add 5 projects for pagination
        for id in 0u64..5 {
            let project = Project {
                id,
                owner: owner.clone(),
                goal: Uint128::from(1000u128 + id as u128 * 100),
                total_donated: Uint128::from(0u128),
                funded: false,
            };
            PROJECTS.save(deps.as_mut().storage, id, &project).unwrap();
        }

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Query the first 2
        let first_page = contract.list_projects(query_ctx, None, Some(2)).unwrap();
        assert_eq!(first_page.len(), 2);
        assert_eq!(first_page[0].id, 0);
        assert_eq!(first_page[1].id, 1);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Query next page using `start_after = 1`
        let second_page = contract.list_projects(query_ctx, Some(1), Some(2)).unwrap();
        assert_eq!(second_page.len(), 2);
        assert_eq!(second_page[0].id, 2);
        assert_eq!(second_page[1].id, 3);

        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));

        // Final page
        let third_page = contract.list_projects(query_ctx, Some(3), Some(2)).unwrap();
        assert_eq!(third_page.len(), 1);
        assert_eq!(third_page[0].id, 4);
    }
}
