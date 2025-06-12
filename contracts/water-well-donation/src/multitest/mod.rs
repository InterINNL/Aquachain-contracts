use crate::contract::UATOM;
use crate::contract::sv::mt::CodeId;
use crate::contract::sv::mt::WaterWellDonationContractProxy;
use crate::errors::ContractError;
use sylvia::cw_multi_test::{BankSudo, IntoAddr, SudoMsg};
use sylvia::cw_std::Coin;
use sylvia::cw_std::Uint128;
use sylvia::cw_std::coin;
use sylvia::multitest::App;

#[test]
fn integration_instantiate_and_create_project() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();
    let goal = Uint128::new(1000);

    contract.create_project(goal).call(&owner).unwrap();

    let project = contract.get_project(1).unwrap();
    assert_eq!(project.goal, goal);
    assert_eq!(project.total_donated, Uint128::zero());
    assert_eq!(project.owner, owner);
    assert!(!project.funded);
}

#[test]
fn integration_donate_to_project() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);

    let owner = "owner".into_addr();
    let donor = "donor".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();
    contract
        .create_project(Uint128::new(1000))
        .call(&owner)
        .unwrap();

    app.app_mut()
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: donor.to_string(),
            amount: vec![coin(400, UATOM)],
        }))
        .unwrap();
    // Donate 400 uatom to project 1
    contract
        .donate(1)
        .with_funds(&[Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(400),
        }])
        .call(&donor)
        .unwrap();

    let project = contract.get_project(1).unwrap();
    assert_eq!(project.total_donated, Uint128::new(400));
    assert!(!project.funded);
}

#[test]
fn create_project_with_zero_goal_should_fail() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let contract = code_id.instantiate().call(&owner).unwrap();

    let err = contract
        .create_project(Uint128::zero())
        .call(&owner)
        .unwrap_err();
    assert!(
        err.to_string()
            .contains(&ContractError::ZeroGoal.to_string())
    );
}

#[test]
fn donate_without_funds_should_fail() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let donor = "donor".into_addr();
    let contract = code_id.instantiate().call(&owner).unwrap();

    contract
        .create_project(Uint128::new(1000))
        .call(&owner)
        .unwrap();

    // Donate without funds
    let err = contract.donate(1).call(&donor).unwrap_err();
    assert!(
        err.to_string()
            .contains(&ContractError::NoDonation.to_string())
    );
}

#[test]
fn donate_exceeding_goal_should_fail() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let donor = "donor".into_addr();
    let contract = code_id.instantiate().call(&owner).unwrap();

    let goal = Uint128::new(1000);
    contract.create_project(goal).call(&owner).unwrap();

    app.app_mut()
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: donor.to_string(),
            amount: vec![coin(1100, UATOM)],
        }))
        .unwrap();

    // Donate 700 uatom first
    contract
        .donate(1)
        .with_funds(&[Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(700),
        }])
        .call(&donor)
        .unwrap();

    // Donate 400 uatom - exceeds goal (700 + 400 = 1100 > 1000)
    let err = contract
        .donate(1)
        .with_funds(&[Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(400),
        }])
        .call(&donor)
        .unwrap_err();

    assert!(
        err.to_string()
            .contains(&ContractError::ExceedGoal.to_string())
    );
}

#[test]
fn disburse_by_owner_after_goal_met() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let donor = "donor".into_addr();
    let contract = code_id.instantiate().call(&owner).unwrap();

    let goal = Uint128::new(1000);
    contract.create_project(goal).call(&owner).unwrap();

    app.app_mut()
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: donor.to_string(),
            amount: vec![coin(goal.into(), UATOM)],
        }))
        .unwrap();

    // Donate full goal amount
    contract
        .donate(1)
        .with_funds(&[Coin {
            denom: UATOM.to_string(),
            amount: goal,
        }])
        .call(&donor)
        .unwrap();

    // Disburse by project owner
    let res = contract.disburse(1).call(&owner).unwrap();
    let attrs = &res.events[1].attributes;

    assert!(
        attrs
            .iter()
            .any(|a| a.key == "action" && a.value == "disburse")
    );
    assert!(
        attrs
            .iter()
            .any(|a| a.key == "project_id" && a.value == "1")
    );
    assert!(
        attrs
            .iter()
            .any(|a| a.key == "recipient" && a.value == owner.to_string())
    );
    assert!(
        attrs
            .iter()
            .any(|a| a.key == "amount" && a.value == goal.to_string())
    );

    // Project should be marked as funded
    let project = contract.get_project(1).unwrap();
    assert!(project.funded);
}

#[test]
fn disburse_by_non_owner_should_fail() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let non_owner = "someone_else".into_addr();
    let donor = "donor".into_addr();
    let contract = code_id.instantiate().call(&owner).unwrap();

    let goal = Uint128::new(1000);
    contract.create_project(goal).call(&owner).unwrap();

    app.app_mut()
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: donor.to_string(),
            amount: vec![coin(goal.into(), UATOM)],
        }))
        .unwrap();

    contract
        .donate(1)
        .with_funds(&[Coin {
            denom: UATOM.to_string(),
            amount: goal,
        }])
        .call(&donor)
        .unwrap();

    // Non-owner tries to disburse
    let err = contract.disburse(1).call(&non_owner).unwrap_err();
    assert!(
        err.to_string()
            .contains(&ContractError::Unauthorized.to_string())
    );
}

#[test]
fn disburse_before_goal_met_should_fail() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let donor = "donor".into_addr();
    let contract = code_id.instantiate().call(&owner).unwrap();

    let goal = Uint128::new(1000);
    contract.create_project(goal).call(&owner).unwrap();

    app.app_mut()
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: donor.to_string(),
            amount: vec![coin(500, UATOM)],
        }))
        .unwrap();

    // Donate less than goal
    contract
        .donate(1)
        .with_funds(&[Coin {
            denom: UATOM.to_string(),
            amount: Uint128::new(500),
        }])
        .call(&donor)
        .unwrap();

    // Owner tries to disburse before goal met
    let err = contract.disburse(1).call(&owner).unwrap_err();
    assert!(
        err.to_string()
            .contains(&ContractError::GoalNotMet.to_string())
    );
}

#[test]
fn disburse_already_disbursed_should_fail() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let donor = "donor".into_addr();
    let contract = code_id.instantiate().call(&owner).unwrap();

    let goal = Uint128::new(1000);
    contract.create_project(goal).call(&owner).unwrap();

    app.app_mut()
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: donor.to_string(),
            amount: vec![coin(goal.into(), UATOM)],
        }))
        .unwrap();

    contract
        .donate(1)
        .with_funds(&[coin(goal.into(), UATOM)])
        .call(&donor)
        .unwrap();

    // First disbursement
    contract.disburse(1).call(&owner).unwrap();

    // Second disbursement should fail
    let err = contract.disburse(1).call(&owner).unwrap_err();
    assert!(
        err.to_string()
            .contains(&ContractError::AlreadyDisbursed.to_string())
    );
}

#[test]
fn list_projects_returns_all_created() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let contract = code_id.instantiate().call(&owner).unwrap();

    // Create multiple projects
    for goal in [500, 1000, 1500] {
        contract
            .create_project(Uint128::new(goal))
            .call(&owner)
            .unwrap();
    }

    let projects = contract.list_projects(None, None).unwrap();
    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0].goal, Uint128::new(500));
    assert_eq!(projects[1].goal, Uint128::new(1000));
    assert_eq!(projects[2].goal, Uint128::new(1500));
}
