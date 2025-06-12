use crate::contract::ATOM;
use crate::contract::REWARD_AMOUNT;
use crate::contract::sv::mt::CitizenScienceRegistryProxy;
use crate::contract::sv::mt::CodeId;
use crate::errors::ContractError;
use sylvia::cw_multi_test::{BankSudo, IntoAddr, SudoMsg};
use sylvia::cw_std::BalanceResponse;
use sylvia::cw_std::BankQuery;
use sylvia::cw_std::QueryRequest;
use sylvia::cw_std::Uint128;
use sylvia::cw_std::coin;
use sylvia::multitest::App;

#[test]
fn integration_instantiate_and_submit_data() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    // Prepare valid JSON data hash
    let valid_json_str = r#"{"temperature": 22, "location": "field"}"#.to_string();
    let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
    // Submit data
    let res = contract.submit_data(valid_json.clone()).call(&owner);
    assert!(res.is_ok(), "submit_data should succeed");

    let entry = contract.get_data_entry(1).unwrap();
    assert_eq!(entry.id, 1);
    assert_eq!(entry.data_str, valid_json.to_string());
    assert_eq!(entry.submitter, owner);
    assert!(!entry.verified);
    assert!(entry.verifier.is_none());
    assert!(!entry.rewarded);
}

#[test]
fn integration_submit_data_fails_on_duplicate() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    let valid_json_str = r#"{"temperature": 22, "location": "field"}"#.to_string();
    let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

    // First submission should succeed
    let res1 = contract.submit_data(valid_json.clone()).call(&owner);
    assert!(res1.is_ok(), "First submit_data should succeed");

    // Second submission of same data should fail
    let res2 = contract.submit_data(valid_json).call(&owner);
    assert!(res2.is_err(), "Second submit_data should fail on duplicate");
    let err = res2.unwrap_err().to_string();
    assert!(err.contains(&ContractError::DuplicateData.to_string()));
}

#[test]
fn integration_submit_data_fails_on_invalid_json() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    let invalid_values = vec![
        serde_json::json!(""),
        serde_json::json!({}),
        serde_json::json!([]),
    ];

    for invalid_json in invalid_values {
        let res = contract.submit_data(invalid_json.clone()).call(&owner);
        assert!(
            res.is_err(),
            "submit_data should fail on invalid JSON: {invalid_json}"
        );
        let err = res.unwrap_err().to_string();
        assert!(
            err.contains(&ContractError::InvalidJson.to_string()),
            "Expected InvalidJson error, got: {err}"
        );
    }
}

#[test]
fn integration_instantiate_and_add_verifier() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let non_owner = "intruder".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    // Add verifier
    let verifier = "verifier1".into_addr();
    let res = contract.add_verifier(verifier.clone()).call(&owner);
    assert!(res.is_ok(), "Owner should be able to add verifier");

    let is_verifier = contract.is_verifier(verifier.clone()).unwrap();
    assert!(is_verifier, "Verifier should be registered");

    // Try adding verifier as non-owner (should fail)
    let another_verifier = "verifier2".into_addr();
    let res = contract
        .add_verifier(another_verifier.clone())
        .call(&non_owner);
    assert!(res.is_err(), "Non-owner should NOT be able to add verifier");
    let err = res.unwrap_err().to_string();
    assert!(err.contains(&ContractError::Unauthorized.to_string()));
}

#[test]
fn integration_verify_data_succeeds_for_registered_verifier() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let verifier = "verifier".into_addr();
    let submitter = "submitter".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    contract
        .add_verifier(verifier.clone())
        .call(&owner)
        .unwrap();

    // Submit valid data by submitter
    let valid_json_str = r#"{"temperature": 22, "location": "field"}"#.to_string();
    let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
    contract.submit_data(valid_json).call(&submitter).unwrap();

    // Verifier verifies the data entry with id = 1
    let res = contract.verify_data(1).call(&verifier);
    assert!(
        res.is_ok(),
        "verify_data should succeed for registered verifier"
    );

    // Check entry updated correctly
    let entry = contract.get_data_entry(1).unwrap();
    assert!(entry.verified);
    assert_eq!(entry.verifier.unwrap(), verifier);
}

#[test]
fn integration_verify_data_fails_for_unregistered_verifier() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let unregistered_verifier = "not_a_verifier".into_addr();
    let submitter = "submitter".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    // Submit valid data by submitter
    let valid_json_str = r#"{"temperature": 22, "location": "field"}"#.to_string();
    let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
    contract.submit_data(valid_json).call(&submitter).unwrap();

    // Attempt to verify by unregistered verifier should fail
    let res = contract.verify_data(1).call(&unregistered_verifier);
    assert!(
        res.is_err(),
        "verify_data should fail for unregistered verifier"
    );
    let err = res.unwrap_err().to_string();
    assert!(err.contains(&ContractError::Unauthorized.to_string()));
}

#[test]
fn integration_verify_data_fails_if_already_verified() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);
    let owner = "owner".into_addr();
    let verifier = "verifier".into_addr();
    let submitter = "submitter".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    contract
        .add_verifier(verifier.clone())
        .call(&owner)
        .unwrap();

    let valid_json_str = r#"{"temperature": 22, "location": "field"}"#.to_string();
    let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
    contract.submit_data(valid_json).call(&submitter).unwrap();

    contract.verify_data(1).call(&verifier).unwrap();

    let res = contract.verify_data(1).call(&verifier);
    assert!(res.is_err(), "verify_data should fail if already verified");
    let err = res.unwrap_err().to_string();
    assert!(err.contains(&ContractError::AlreadyVerified.to_string()));
}

#[test]
fn integration_reward_contributor_succeeds_for_verified_entry() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);

    let owner = "owner".into_addr();
    let verifier = "verifier".into_addr();
    let contributor = "contributor".into_addr();

    // Fund reward funds to owner so they can pay the reward
    app.app_mut()
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: owner.to_string(),
            amount: vec![coin(REWARD_AMOUNT, ATOM)],
        }))
        .unwrap();

    let contract = code_id.instantiate().call(&owner).unwrap();

    contract
        .add_verifier(verifier.clone())
        .call(&owner)
        .unwrap();

    let valid_json_str = r#"{"observation": "species A", "count": 5}"#.to_string();
    let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

    contract.submit_data(valid_json).call(&contributor).unwrap();

    contract.verify_data(1).call(&verifier).unwrap();

    let balance_before = ((*app.app())
        .wrap()
        .query::<BalanceResponse>(&QueryRequest::Bank(BankQuery::Balance {
            address: contributor.to_string(),
            denom: ATOM.to_string(),
        }))
        .unwrap())
    .amount;

    // Now reward contributor by sending the reward funds with the call
    let res = contract
        .reward_contributor(1)
        .with_funds(&[coin(REWARD_AMOUNT, ATOM)])
        .call(&owner);

    assert!(
        res.is_ok(),
        "reward_contributor should succeed for verified entry"
    );

    let entry = contract.get_data_entry(1).unwrap();
    assert!(entry.rewarded);

    assert!(
        res.unwrap().events.iter().any(|event| {
            event.ty == "wasm"
                && event
                    .attributes
                    .iter()
                    .any(|attr| attr.key == "action" && attr.value == "reward_contributor")
        }),
        "Expected reward_contributor event"
    );

    let balance_after = ((*app.app())
        .wrap()
        .query::<BalanceResponse>(&QueryRequest::Bank(BankQuery::Balance {
            address: contributor.to_string(),
            denom: ATOM.to_string(),
        }))
        .unwrap())
    .amount;

    assert_eq!(
        balance_after.amount,
        balance_before.amount + Uint128::from(REWARD_AMOUNT),
        "Balance should increase by reward amount"
    );

    assert_eq!(
        balance_after.denom, balance_before.denom,
        "Denomination should stay the same"
    );
}

#[test]
fn integration_reward_contributor_fails_if_not_verified() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);

    let owner = "owner".into_addr();
    let contributor = "contributor".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    let valid_json_str = r#"{"observation": "species B", "count": 3}"#.to_string();
    let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

    contract.submit_data(valid_json).call(&contributor).unwrap();

    // Attempt to reward contributor before verification
    let res = contract.reward_contributor(1).call(&owner);
    assert!(
        res.is_err(),
        "reward_contributor should fail if not verified"
    );

    let err = res.unwrap_err().to_string();
    assert!(err.contains(&ContractError::NotVerified.to_string()));
}

#[test]
fn integration_reward_contributor_fails_if_already_rewarded() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);

    let owner = "owner".into_addr();
    let verifier = "verifier".into_addr();
    let contributor = "contributor".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    contract
        .add_verifier(verifier.clone())
        .call(&owner)
        .unwrap();

    let valid_json_str = r#"{"observation": "species C", "count": 1}"#.to_string();
    let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

    contract.submit_data(valid_json).call(&contributor).unwrap();
    contract.verify_data(1).call(&verifier).unwrap();

    // Fund reward 2 times funds to owner so they can pay the reward
    app.app_mut()
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: owner.to_string(),
            amount: vec![coin(REWARD_AMOUNT * 2, ATOM)],
        }))
        .unwrap();

    // First reward call succeeds
    let _ = contract
        .reward_contributor(1)
        .with_funds(&[coin(REWARD_AMOUNT, ATOM)])
        .call(&owner);

    // Second call should fail
    let res = contract
        .reward_contributor(1)
        .with_funds(&[coin(REWARD_AMOUNT, ATOM)])
        .call(&owner);

    assert!(
        res.is_err(),
        "reward_contributor should fail if already rewarded"
    );

    let err = res.unwrap_err().to_string();
    assert!(err.contains(&ContractError::AlreadyRewarded.to_string()));
}

#[test]
fn integration_get_data_entry_returns_correct_entry() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);

    let owner = "owner".into_addr();
    let verifier = "verifier".into_addr();
    let contributor = "contributor".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();
    contract
        .add_verifier(verifier.clone())
        .call(&owner)
        .unwrap();

    let valid_json_str = r#"{"observation": "species A", "count": 5}"#.to_string();
    let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

    contract
        .submit_data(valid_json.clone())
        .call(&contributor)
        .unwrap();

    contract.verify_data(1).call(&verifier).unwrap();

    // Query the data entry
    let entry = contract.get_data_entry(1).unwrap();

    // Assert
    assert_eq!(entry.id, 1);
    assert_eq!(entry.data_str, valid_json.to_string());
    assert_eq!(entry.submitter, contributor);
    assert!(entry.verified);
    assert_eq!(entry.verifier, Some(verifier));
    assert!(!entry.rewarded);
}

#[test]
fn integration_list_data_entries_returns_correct_range() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);

    let owner = "owner".into_addr();
    let contributor = "contributor".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    // Submit 3 entries
    let data_entries = vec![
        r#"{"species": "A", "count": 1}"#,
        r#"{"species": "B", "count": 2}"#,
        r#"{"species": "C", "count": 3}"#,
    ];

    for valid_json_str in &data_entries {
        let valid_json: serde_json::Value = serde_json::from_str(valid_json_str).unwrap();
        contract.submit_data(valid_json).call(&contributor).unwrap();
    }

    // Query all 3
    let entries = contract.list_data_entries(None, Some(10)).unwrap();
    assert_eq!(entries.len(), 3);
    for (i, entry) in entries.iter().enumerate() {
        let expected_data_str = serde_json::to_string(
            &serde_json::from_str::<serde_json::Value>(data_entries[i]).unwrap(),
        )
        .unwrap();

        assert_eq!(entry.id, (i + 1) as u64);
        assert_eq!(entry.data_str, expected_data_str);
        assert_eq!(entry.submitter, contributor);
    }

    // Query with pagination (start_after = 1)
    let entries_page = contract.list_data_entries(Some(1), Some(1)).unwrap();
    assert_eq!(entries_page.len(), 1);

    let expected_data_str =
        serde_json::to_string(&serde_json::from_str::<serde_json::Value>(data_entries[1]).unwrap())
            .unwrap();

    assert_eq!(entries_page[0].id, 2);
    assert_eq!(entries_page[0].data_str, expected_data_str);
}

#[test]
fn integration_list_verifiers_returns_all_verifiers() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);

    let owner = "owner".into_addr();
    let verifier1 = "verifier1".into_addr();
    let verifier2 = "verifier2".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    // Add verifiers
    contract
        .add_verifier(verifier1.clone())
        .call(&owner)
        .unwrap();
    contract
        .add_verifier(verifier2.clone())
        .call(&owner)
        .unwrap();

    // Query list of verifiers
    let verifiers = contract.list_verifiers().unwrap();

    // Ensure both verifiers are returned
    assert_eq!(verifiers.len(), 2);
    assert!(verifiers.contains(&verifier1));
    assert!(verifiers.contains(&verifier2));
}

#[test]
fn integration_is_verifier_returns_correct_status() {
    let app = App::default();
    let code_id = CodeId::store_code(&app);

    let owner = "owner".into_addr();
    let verifier = "verifier".into_addr();
    let non_verifier = "someone_else".into_addr();

    let contract = code_id.instantiate().call(&owner).unwrap();

    // Initially, no one is a verifier
    let res = contract.is_verifier(verifier.clone()).unwrap();
    assert!(!res, "verifier should not be listed initially");

    // Add a verifier
    contract
        .add_verifier(verifier.clone())
        .call(&owner)
        .unwrap();

    // Now `verifier` should return true
    let res = contract.is_verifier(verifier.clone()).unwrap();
    assert!(res, "verifier should be listed after being added");

    // Another address should still return false
    let res = contract.is_verifier(non_verifier.clone()).unwrap();
    assert!(!res, "non-verifier should not be listed");
}
