#[cfg(test)]
mod tests {
    use crate::constants::DEFAULT_DENOM;
    use crate::contract::sv::mt::CodeId;
    use crate::contract::sv::mt::WaterWellInitiativeContractProxy;
    use crate::contract::valid_json_str;
    use crate::enums::ProjectStatus;
    use crate::errors::ContractError;
    use serde_json::Value;
    use sylvia::cw_multi_test::{BankSudo, IntoAddr, SudoMsg};
    use sylvia::cw_std::Coin;
    use sylvia::cw_std::Uint128;
    use sylvia::cw_std::coin;
    use sylvia::multitest::App;

    #[test]
    fn integration_instantiate_and_create_project() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        let goal = Uint128::new(1000);

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(goal, valid_json)
            .call(&owner)
            .unwrap();

        let project = contract.get_project(1).unwrap();
        assert_eq!(project.goal, goal);
        assert_eq!(project.total_donated, Uint128::zero());
        assert_eq!(project.owner, owner);
        assert!(project.status == ProjectStatus::Proposed);
    }

    #[test]
    fn integration_get_project() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        // Instanciation du contrat
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let goal = Uint128::new(1000);
        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        // Création d'un projet
        contract
            .create_project(goal, valid_json)
            .call(&owner)
            .unwrap();

        // Query pour récupérer le projet par son id
        let project = contract.get_project(1).unwrap();

        // Assert que la query retourne bien le projet créé
        assert_eq!(project.id, 1);
        assert_eq!(project.goal, goal);
        assert_eq!(project.owner, owner);
        assert_eq!(project.status, ProjectStatus::Proposed);
        assert_eq!(project.total_donated, Uint128::zero());
    }

    #[test]
    fn integration_create_project_with_zero_goal_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let err = contract
            .create_project(Uint128::zero(), valid_json)
            .call(&owner)
            .unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::ZeroGoal.to_string())
        );
    }

    #[test]
    fn integration_create_project_fails_on_invalid_json() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        let goal = Uint128::new(1000);

        let invalid_values = vec![
            serde_json::json!(""),
            serde_json::json!({}),
            serde_json::json!([]),
        ];

        for invalid_json in invalid_values {
            let res = contract
                .create_project(goal, invalid_json.clone())
                .call(&owner);
            assert!(
                res.is_err(),
                "create_project should fail on invalid JSON: {invalid_json}"
            );
            let err = res.unwrap_err().to_string();
            assert!(
                err.contains(&ContractError::InvalidJson.to_string()),
                "Expected InvalidJson error, got: {err}"
            );
        }
    }

    #[test]
    fn integration_create_project_fails_on_duplicate() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        let goal = Uint128::new(1000);

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        // First project creation should succeed
        let res1 = contract
            .create_project(goal, valid_json.clone())
            .call(&owner);
        assert!(res1.is_ok(), "First project creation should succeed");

        // Second project creation with identical JSON should fail (due to hash collision)
        let res2 = contract.create_project(goal, valid_json).call(&owner);
        assert!(
            res2.is_err(),
            "Second project creation should fail on duplicate"
        );
        let err = res2.unwrap_err().to_string();
        assert!(
            err.contains(&ContractError::DuplicateData.to_string()),
            "Expected DuplicateData error, got: {err}"
        );
    }

    #[test]
    fn integration_validate_project() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();

        // Admin validates the project
        let res = contract.validate(1).call(&admin).unwrap();

        // Check that project is marked as fundraising
        let project = contract.get_project(1).unwrap();
        assert!(project.status == ProjectStatus::Fundraising);

        // Extract attributes from the second event
        let attrs = &res.events[1].attributes;

        assert!(
            attrs
                .iter()
                .any(|a| a.key == "action" && a.value == "validate"),
            "Missing or incorrect 'action' attribute"
        );
        assert!(
            attrs
                .iter()
                .any(|a| a.key == "project_id" && a.value == "1"),
            "Missing or incorrect 'project_id' attribute"
        );
        assert!(
            attrs
                .iter()
                .any(|a| a.key == "admin" && a.value == admin.to_string()),
            "Missing or incorrect 'admin' attribute"
        );
        assert!(
            attrs
                .iter()
                .any(|a| a.key == "owner" && a.value == owner.to_string()),
            "Missing or incorrect 'owner' attribute"
        );
    }

    #[test]
    fn integration_donate_to_project() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();

        // Admin validates the project
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(400, DEFAULT_DENOM)],
            }))
            .unwrap();
        // Donate 400 uatom to project 1
        contract
            .donate(1)
            .with_funds(&[Coin {
                denom: DEFAULT_DENOM.to_string(),
                amount: Uint128::new(400),
            }])
            .call(&donor)
            .unwrap();

        let project = contract.get_project(1).unwrap();
        assert_eq!(project.total_donated, Uint128::new(400));
        assert!(project.status == ProjectStatus::Fundraising);
    }

    #[test]
    fn integration_validate_by_non_admin_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let non_admin = "not_admin".into_addr();
        let owner = "owner".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();

        let err = contract.validate(1).call(&non_admin).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::Unauthorized.to_string())
        );
    }

    #[test]
    fn integration_double_validation_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();
        contract.validate(1).call(&admin).unwrap();

        let err = contract.validate(1).call(&admin).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::AlreadyValidated.to_string())
        );
    }

    #[test]
    fn integration_validate_nonexistent_project_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let err = contract.validate(999).call(&admin).unwrap_err();

        dbg!(err.to_string());

        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn integration_donate_without_funds_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();

        // Admin validates the project
        contract.validate(1).call(&admin).unwrap();

        // Donate without funds
        let err = contract.donate(1).call(&donor).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::NoDonation.to_string())
        );
    }

    #[test]
    fn integration_donate_exceeding_goal_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let goal = Uint128::new(1000);
        contract
            .create_project(goal, valid_json)
            .call(&owner)
            .unwrap();

        // Admin validates the project
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(1100, DEFAULT_DENOM)],
            }))
            .unwrap();

        // Donate 700 uatom first
        contract
            .donate(1)
            .with_funds(&[Coin {
                denom: DEFAULT_DENOM.to_string(),
                amount: Uint128::new(700),
            }])
            .call(&donor)
            .unwrap();

        // Donate 400 uatom - exceeds goal (700 + 400 = 1100 > 1000)
        let err = contract
            .donate(1)
            .with_funds(&[Coin {
                denom: DEFAULT_DENOM.to_string(),
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
    fn integration_donate_after_completion_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(1000, DEFAULT_DENOM)],
            }))
            .unwrap();

        contract
            .donate(1)
            .with_funds(&[coin(1000, DEFAULT_DENOM)])
            .call(&donor)
            .unwrap();

        contract.unlock(1).call(&admin).unwrap();
        contract.disburse(1).call(&owner).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(1000, DEFAULT_DENOM)],
            }))
            .unwrap();

        let err = contract
            .donate(1)
            .with_funds(&[coin(100, DEFAULT_DENOM)])
            .call(&donor)
            .unwrap_err();

        assert!(
            err.to_string()
                .contains(&ContractError::AlreadyCompleted.to_string())
        );
    }

    #[test]
    fn integration_multiple_donors_should_accumulate_donations() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor1 = "donor1".into_addr();
        let donor2 = "donor2".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor1.to_string(),
                amount: vec![coin(600, DEFAULT_DENOM)],
            }))
            .unwrap();
        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor2.to_string(),
                amount: vec![coin(400, DEFAULT_DENOM)],
            }))
            .unwrap();

        contract
            .donate(1)
            .with_funds(&[coin(600, DEFAULT_DENOM)])
            .call(&donor1)
            .unwrap();
        contract
            .donate(1)
            .with_funds(&[coin(400, DEFAULT_DENOM)])
            .call(&donor2)
            .unwrap();

        let project = contract.get_project(1).unwrap();
        assert_eq!(project.total_donated, Uint128::new(1000));
    }

    #[test]
    fn integration_cancel_by_owner_in_proposed_should_succeed() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();

        // Owner cancels project (still Proposed)
        contract.cancel(1).call(&owner).unwrap();

        let project = contract.get_project(1).unwrap();
        assert_eq!(project.status, ProjectStatus::Cancelled);
    }

    #[test]
    fn integration_cancel_by_admin_in_fundraising_should_succeed() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();

        contract.validate(1).call(&admin).unwrap();

        contract.cancel(1).call(&admin).unwrap();

        let project = contract.get_project(1).unwrap();
        assert_eq!(project.status, ProjectStatus::Cancelled);
    }

    #[test]
    fn integration_cancel_by_stranger_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let stranger = "stranger".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();

        let err = contract.cancel(1).call(&stranger).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::Unauthorized.to_string())
        );
    }

    #[test]
    fn integration_cancel_already_cancelled_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();

        contract.cancel(1).call(&owner).unwrap();

        let err = contract.cancel(1).call(&admin).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::AlreadyCancelled.to_string())
        );
    }

    #[test]
    fn integration_cancel_invalid_state_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        contract
            .create_project(Uint128::new(1000), valid_json.clone())
            .call(&owner)
            .unwrap();
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(1000, DEFAULT_DENOM)],
            }))
            .unwrap();

        contract
            .donate(1)
            .with_funds(&[coin(1000, DEFAULT_DENOM)])
            .call(&donor)
            .unwrap();

        contract.unlock(1).call(&admin).unwrap();
        contract.disburse(1).call(&owner).unwrap();

        let err = contract.cancel(1).call(&admin).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::CannotCancel.to_string())
        );
    }

    #[test]
    fn integration_refund_success() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json.clone())
            .call(&owner)
            .unwrap();

        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(500, DEFAULT_DENOM)],
            }))
            .unwrap();

        // Donor donates 500
        contract
            .donate(1)
            .with_funds(&[coin(500, DEFAULT_DENOM)])
            .call(&donor)
            .unwrap();

        // Admin cancels the project
        contract.cancel(1).call(&admin).unwrap();

        // Donor refunds
        let res = contract.refund(1).call(&donor).unwrap();

        let attrs = &res.events[1].attributes;

        assert!(
            attrs
                .iter()
                .any(|a| a.key == "action" && a.value == "refund")
        );
        assert!(
            attrs
                .iter()
                .any(|a| a.key == "project_id" && a.value == "1")
        );
        assert!(
            attrs
                .iter()
                .any(|a| a.key == "refunded_to" && a.value == donor.to_string())
        );
        assert!(attrs.iter().any(|a| a.key == "amount" && a.value == "500"));
        assert!(
            attrs
                .iter()
                .any(|a| a.key == "status" && a.value == "Cancelled")
        );
    }

    #[test]
    fn integration_refund_fail_no_donation() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json.clone())
            .call(&owner)
            .unwrap();

        // Cancel project without donations
        contract.cancel(1).call(&admin).unwrap();

        // Donor tries to refund with no donation
        let err = contract.refund(1).call(&donor).unwrap_err();

        assert!(
            err.to_string()
                .contains(&ContractError::NoRefundAvailable.to_string())
        );
    }

    #[test]
    fn integration_refund_fail_not_cancelled() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json.clone())
            .call(&owner)
            .unwrap();

        // Validate project
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(300, DEFAULT_DENOM)],
            }))
            .unwrap();

        // Donor donates 300
        contract
            .donate(1)
            .with_funds(&[coin(300, DEFAULT_DENOM)])
            .call(&donor)
            .unwrap();

        // Donor tries to refund while project is NOT cancelled
        let err = contract.refund(1).call(&donor).unwrap_err();

        assert!(
            err.to_string()
                .contains(&ContractError::NotRefundable.to_string())
        );
    }

    #[test]
    fn integration_unlock_by_non_admin_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let non_admin = "not_admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(1000, DEFAULT_DENOM)],
            }))
            .unwrap();

        contract
            .donate(1)
            .with_funds(&[coin(1000, DEFAULT_DENOM)])
            .call(&donor)
            .unwrap();

        let err = contract.unlock(1).call(&non_admin).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::Unauthorized.to_string())
        );
    }

    #[test]
    fn integration_double_unlock_should_fail_or_be_idempotent() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(1000, DEFAULT_DENOM)],
            }))
            .unwrap();

        contract
            .donate(1)
            .with_funds(&[coin(1000, DEFAULT_DENOM)])
            .call(&donor)
            .unwrap();
        contract.unlock(1).call(&admin).unwrap();

        let err = contract.unlock(1).call(&admin).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::AlreadyDisbursable.to_string())
        );
    }

    #[test]
    fn integration_unlock_before_validation_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();

        let err = contract.unlock(1).call(&admin).unwrap_err();
        dbg!(err.to_string());
        assert!(
            err.to_string()
                .contains(&ContractError::GoalNotMet.to_string())
        );
    }

    #[test]
    fn integration_disburse_by_owner_after_goal_met() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let goal = Uint128::new(1000);
        contract
            .create_project(goal, valid_json)
            .call(&owner)
            .unwrap();

        // Admin validates the project
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(goal.into(), DEFAULT_DENOM)],
            }))
            .unwrap();

        // Donate full goal amount
        contract
            .donate(1)
            .with_funds(&[Coin {
                denom: DEFAULT_DENOM.to_string(),
                amount: goal,
            }])
            .call(&donor)
            .unwrap();

        // Admin unlocks funds
        contract.unlock(1).call(&admin).unwrap();

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
                .any(|a| a.key == "sender" && a.value == owner.to_string())
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

        let project = contract.get_project(1).unwrap();
        assert!(project.status == ProjectStatus::Completed);
    }

    #[test]
    fn integration_disburse_by_admin_after_goal_met() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let goal = Uint128::new(1000);
        contract
            .create_project(goal, valid_json)
            .call(&owner)
            .unwrap();

        // Admin validates the project
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(goal.into(), DEFAULT_DENOM)],
            }))
            .unwrap();

        // Donate full goal amount
        contract
            .donate(1)
            .with_funds(&[Coin {
                denom: DEFAULT_DENOM.to_string(),
                amount: goal,
            }])
            .call(&donor)
            .unwrap();

        // Admin unlocks funds
        contract.unlock(1).call(&admin).unwrap();

        // Disburse by admin
        let res = contract.disburse(1).call(&admin).unwrap();
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
                .any(|a| a.key == "sender" && a.value == admin.to_string())
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
        assert!(project.status == ProjectStatus::Completed);
    }

    #[test]
    fn integration_disburse_by_non_owner_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let non_owner = "someone_else".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let goal = Uint128::new(1000);
        contract
            .create_project(goal, valid_json)
            .call(&owner)
            .unwrap();

        // Admin validates the project
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(goal.into(), DEFAULT_DENOM)],
            }))
            .unwrap();

        contract
            .donate(1)
            .with_funds(&[Coin {
                denom: DEFAULT_DENOM.to_string(),
                amount: goal,
            }])
            .call(&donor)
            .unwrap();

        // Admin unlocks funds
        contract.unlock(1).call(&admin).unwrap();

        // Non-owner tries to disburse
        let err = contract.disburse(1).call(&non_owner).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::Unauthorized.to_string())
        );
    }

    #[test]
    fn integration_disburse_before_goal_met_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let goal = Uint128::new(1000);
        contract
            .create_project(goal, valid_json)
            .call(&owner)
            .unwrap();

        // Admin validates the project
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(500, DEFAULT_DENOM)],
            }))
            .unwrap();

        // Donate less than goal
        contract
            .donate(1)
            .with_funds(&[Coin {
                denom: DEFAULT_DENOM.to_string(),
                amount: Uint128::new(500),
            }])
            .call(&donor)
            .unwrap();

        // Admin tries to unlock funds
        let err = contract.unlock(1).call(&admin).unwrap_err();

        assert!(
            err.to_string()
                .contains(&ContractError::GoalNotMet.to_string())
        );

        // Owner tries to disburse before goal met
        let err = contract.disburse(1).call(&owner).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::NotDisbursable.to_string())
        );
    }

    #[test]
    fn integration_disburse_already_disbursed_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let goal = Uint128::new(1000);
        contract
            .create_project(goal, valid_json)
            .call(&owner)
            .unwrap();

        // Admin validates the project
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(goal.into(), DEFAULT_DENOM)],
            }))
            .unwrap();

        contract
            .donate(1)
            .with_funds(&[coin(goal.into(), DEFAULT_DENOM)])
            .call(&donor)
            .unwrap();

        // Admin unlocks funds
        contract.unlock(1).call(&admin).unwrap();

        // First disbursement
        contract.disburse(1).call(&owner).unwrap();

        // Second disbursement should fail
        let err = contract.disburse(1).call(&owner).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::AlreadyCompleted.to_string())
        );
    }

    #[test]
    fn integration_disburse_before_unlock_should_fail() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .create_project(Uint128::new(1000), valid_json)
            .call(&owner)
            .unwrap();
        contract.validate(1).call(&admin).unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: donor.to_string(),
                amount: vec![coin(1000, DEFAULT_DENOM)],
            }))
            .unwrap();

        contract
            .donate(1)
            .with_funds(&[coin(1000, DEFAULT_DENOM)])
            .call(&donor)
            .unwrap();

        let err = contract.disburse(1).call(&owner).unwrap_err();
        assert!(
            err.to_string()
                .contains(&ContractError::NotDisbursable.to_string())
        );
    }

    #[test]
    fn integration_list_projects_returns_all_created() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Create multiple projects
        for (i, goal) in [500, 1000, 1500].iter().enumerate() {
            let project_id = (i + 1) as u64; // project IDs start at 1

            let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

            contract
                .create_project(Uint128::new(*goal), valid_json.clone())
                .call(&owner)
                .unwrap();
            contract.validate(project_id).call(&admin).unwrap();
        }

        let projects = contract.list_projects(None, None).unwrap();
        assert_eq!(projects.len(), 3);
        assert_eq!(projects[0].goal, Uint128::new(500));
        assert_eq!(projects[1].goal, Uint128::new(1000));
        assert_eq!(projects[2].goal, Uint128::new(1500));
    }

    #[test]
    fn integration_list_projects_with_pagination() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        for goal in [500, 1000, 1500, 2000, 2500] {
            let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();
            contract
                .create_project(Uint128::new(goal), valid_json.clone())
                .call(&owner)
                .unwrap();
        }

        let first_page = contract.list_projects(Some(2), None).unwrap(); // IDs 1, 2
        let second_page = contract.list_projects(Some(2), Some(2)).unwrap(); // IDs 3, 4
        let third_page = contract.list_projects(Some(2), Some(4)).unwrap(); // ID 5

        assert_eq!(first_page.len(), 2);
        assert_eq!(second_page.len(), 2);
        assert_eq!(third_page.len(), 1);
    }

    #[test]
    fn integration_multiple_projects_status_counts() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let donor = "donor".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Create 8 projects, IDs 1 to 8
        for _ in 0..8 {
            let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();
            contract
                .create_project(Uint128::new(1000), valid_json.clone())
                .call(&owner)
                .unwrap();
        }

        // IDs 1 and 2 stay Proposed (no change)

        // IDs 3 and 4 => Fundraising (validated)
        for id in 3..=4 {
            contract.validate(id).call(&admin).unwrap();
        }

        // IDs 5 and 6 => Funded by validating and donating full goal
        for id in 5..=6 {
            contract.validate(id).call(&admin).unwrap();

            app.app_mut()
                .sudo(SudoMsg::Bank(BankSudo::Mint {
                    to_address: donor.to_string(),
                    amount: vec![coin(1000, DEFAULT_DENOM)],
                }))
                .unwrap();

            contract
                .donate(id)
                .with_funds(&[Coin {
                    denom: DEFAULT_DENOM.to_string(),
                    amount: Uint128::new(1000),
                }])
                .call(&donor)
                .unwrap();
        }

        // ID 7 => Disbursable by validating, donating, unlocking
        {
            let id = 7;
            contract.validate(id).call(&admin).unwrap();

            app.app_mut()
                .sudo(SudoMsg::Bank(BankSudo::Mint {
                    to_address: donor.to_string(),
                    amount: vec![coin(1000, DEFAULT_DENOM)],
                }))
                .unwrap();

            contract
                .donate(id)
                .with_funds(&[Coin {
                    denom: DEFAULT_DENOM.to_string(),
                    amount: Uint128::new(1000),
                }])
                .call(&donor)
                .unwrap();

            contract.unlock(id).call(&admin).unwrap();
        }

        // ID 8 => Completed by validating, donating, unlocking, disbursing
        {
            let id = 8;
            contract.validate(id).call(&admin).unwrap();

            app.app_mut()
                .sudo(SudoMsg::Bank(BankSudo::Mint {
                    to_address: donor.to_string(),
                    amount: vec![coin(1000, DEFAULT_DENOM)],
                }))
                .unwrap();

            contract
                .donate(id)
                .with_funds(&[Coin {
                    denom: DEFAULT_DENOM.to_string(),
                    amount: Uint128::new(1000),
                }])
                .call(&donor)
                .unwrap();

            contract.unlock(id).call(&admin).unwrap();

            contract.disburse(id).call(&owner).unwrap();
        }

        // Query project statuses counts
        let counts = contract.get_project_status_counts().unwrap();

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
    fn integration_test_get_projects_by_status() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Create 3 projects, all start as Proposed
        for _ in 0..3 {
            let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();

            contract
                .create_project(Uint128::new(1000), valid_json.clone())
                .call(&owner)
                .unwrap();
        }

        // Validate 2 projects to move them to Fundraising
        for project_id in 1..=2 {
            contract.validate(project_id).call(&admin).unwrap();
        }

        // Query Proposed projects (should be 1 project left Proposed)
        let proposed_projects = contract
            .get_projects_by_status(ProjectStatus::Proposed, None, None)
            .unwrap();
        assert_eq!(proposed_projects.len(), 1);
        assert_eq!(proposed_projects[0].status, ProjectStatus::Proposed);

        // Query Fundraising projects (2 projects validated)
        let fundraising_projects = contract
            .get_projects_by_status(ProjectStatus::Fundraising, None, None)
            .unwrap();
        assert_eq!(fundraising_projects.len(), 2);
        for project in fundraising_projects.iter() {
            assert_eq!(project.status, ProjectStatus::Fundraising);
        }

        // Query Funded projects (none yet)
        let funded_projects = contract
            .get_projects_by_status(ProjectStatus::Funded, None, None)
            .unwrap();
        assert_eq!(funded_projects.len(), 0);
    }

    #[test]
    fn integration_test_get_projects_by_status_with_pagination() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let owner = "owner".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Create 5 projects — all default to Proposed
        for _ in 0..5 {
            let valid_json: Value = serde_json::from_str(&valid_json_str()).unwrap();
            contract
                .create_project(Uint128::new(1000), valid_json.clone())
                .call(&owner)
                .unwrap();
        }

        // Validate 3 of them so they become Fundraising
        for id in 1..=3 {
            contract.validate(id).call(&admin).unwrap();
        }

        // Query Fundraising projects, paginated
        let page_1 = contract
            .get_projects_by_status(ProjectStatus::Fundraising, Some(2), None)
            .unwrap(); // IDs 1, 2

        let page_2 = contract
            .get_projects_by_status(ProjectStatus::Fundraising, Some(2), Some(2))
            .unwrap(); // ID 3

        assert_eq!(page_1.len(), 2);
        assert_eq!(page_1[0].id, 1);
        assert_eq!(page_1[1].id, 2);

        assert_eq!(page_2.len(), 1);
        assert_eq!(page_2[0].id, 3);
    }
}
