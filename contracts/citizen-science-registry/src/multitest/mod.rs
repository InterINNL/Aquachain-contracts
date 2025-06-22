#[cfg(test)]
mod tests {
    use crate::constants::DEFAULT_DENOM;
    use crate::contract::sv::mt::CitizenScienceRegistryProxy;
    use crate::contract::sv::mt::CodeId;
    use crate::contract::tests::REWARD_AMOUNT;
    use crate::errors::ContractError;
    use serde_json::json;
    use sylvia::cw_multi_test::{BankSudo, IntoAddr, SudoMsg};
    use sylvia::cw_std::BalanceResponse;
    use sylvia::cw_std::BankQuery;
    use sylvia::cw_std::QueryRequest;
    use sylvia::cw_std::Uint128;
    use sylvia::cw_std::coin;
    use sylvia::multitest::App;

    #[test]
    fn integration_instantiate_and_sets_admin_correctly() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let creator = "deployer".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&creator)
            .unwrap();

        // Query internal admin storage via custom query if exposed,
        // or just try privileged action to verify
        let res = contract.add_verifier("someone".into_addr()).call(&creator);
        assert!(
            res.is_ok(),
            "Creator should be stored as admin and allowed to add verifiers"
        );
    }

    #[test]
    fn integration_submit_sensor_succeeds_for_valid_json() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let user = "user".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let sensor_data = json!({
            "name": "Temperature Sensor",
            "location": "Greenhouse 3",
            "meta": { "model": "T-1000", "units": "Celsius" }
        });

        let res = contract.submit_sensor(sensor_data.clone()).call(&user);
        assert!(res.is_ok(), "Valid sensor data should be accepted");

        let sensor = contract.get_sensor(1).unwrap();
        assert_eq!(sensor.data_str, sensor_data.to_string());
        assert_eq!(sensor.owner, user);
    }

    #[test]
    fn integration_submit_sensor_fails_on_invalid_json() {
        let app = App::default();
        let admin = "admin".into_addr();
        let user = "user".into_addr();
        let code_id = CodeId::store_code(&app);
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let invalid_jsons = vec![
            json!(""), // empty string
            json!({}), // empty object
            json!([]), // empty array
        ];

        for data in invalid_jsons {
            let res = contract.submit_sensor(data).call(&user);
            assert!(res.is_err(), "Expected failure for invalid JSON");
        }
    }

    #[test]
    fn integration_activate_sensor_once_only() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let submitter = "submitter".into_addr();

        // Submit a sensor
        let sensor_json = serde_json::json!({
            "name": "forest",
            "location": "north"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // First activation should succeed
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "First activate should succeed");

        // Second activation should fail with AlreadyActivated
        let res = contract.activate(1).call(&admin);
        assert!(res.is_err(), "Second activate should fail");
        let err = res.unwrap_err().to_string();
        assert!(
            err.contains(&ContractError::AlreadyActivated.to_string()),
            "Expected AlreadyActivated error, got: {err}"
        );
    }

    #[test]
    fn integration_activate_sensor_fails_for_non_admin() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let not_admin = "alice".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let sensor = json!({ "name": "sensor", "loc": "x" });
        contract.submit_sensor(sensor).call(&not_admin).unwrap();

        let res = contract.activate(1).call(&not_admin);
        assert!(res.is_err(), "Only admin should be able to activate");
    }

    #[test]
    fn integration_deactivate_sensor() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let non_admin = "intruder".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor
        let sensor_json = serde_json::json!({
            "name": "valley",
            "location": "south"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor first
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "activate should succeed");

        // Deactivate the sensor (as admin)
        let res = contract.deactivate(1).call(&admin);
        assert!(res.is_ok(), "deactivate should succeed");

        // Check that the sensor is now inactive
        let sensor = contract.get_sensor(1).unwrap();
        assert_eq!(sensor.status.to_string(), "Inactive");

        // Attempt to deactivate again (as non-admin) — should fail
        let res = contract.deactivate(1).call(&non_admin);
        assert!(res.is_err(), "non-admin should not be able to deactivate");
        let err = res.unwrap_err().to_string();
        assert!(
            err.contains(&ContractError::Unauthorized.to_string()),
            "Expected Unauthorized error, got: {err}"
        );
    }

    #[test]
    fn integration_deactivate_sensor_twice_is_ok() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let sensor = json!({ "foo": "bar" });
        contract.submit_sensor(sensor).call(&admin).unwrap();

        contract.activate(1).call(&admin).unwrap();
        contract.deactivate(1).call(&admin).unwrap();

        // Second deactivate should succeed (idempotent)
        let res = contract.deactivate(1).call(&admin);
        assert!(
            res.is_ok(),
            "Deactivating already inactive sensor should succeed"
        );
    }

    #[test]
    fn integration_add_verifier_fails_for_non_admin() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let user = "user".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        let res = contract.add_verifier(user.clone()).call(&user);
        assert!(res.is_err());
    }

    #[test]
    fn integration_add_verifier_twice_fails() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let verifier = "verifier1".into_addr();

        // First addition (should succeed)
        let res = contract.add_verifier(verifier.clone()).call(&admin);
        assert!(res.is_ok(), "First add_verifier should succeed");

        // Second addition (should fail)
        let res = contract.add_verifier(verifier.clone()).call(&admin);
        assert!(res.is_err(), "Second add_verifier should fail");
        let err = res.unwrap_err().to_string();

        assert!(
            err.contains(&ContractError::VerifierAlreadyExists.to_string()),
            "Expected VerifierAlreadyExists error, got: {err}"
        );
    }

    #[test]
    fn integration_submit_data() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        // Submit valid data (as submitter)
        let valid_json = serde_json::json!({
            "temperature": 22,
            "location": "field"
        });
        let res = contract.submit_data(1, valid_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_data should succeed");

        // Read back the entry
        let entry = contract.get_data_entry(1).unwrap();
        assert_eq!(entry.id, 1);
        assert_eq!(entry.data_str, valid_json.to_string());
        assert_eq!(entry.submitter, submitter);
        assert!(!entry.verified);
        assert!(entry.verifier.is_none());
        assert!(!entry.rewarded);
    }

    #[test]
    fn integration_submit_data_fails_on_duplicate() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        let valid_json_str = r#"{"temperature": 22, "location": "field"}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

        // First submission should succeed
        let res1 = contract.submit_data(1, valid_json.clone()).call(&submitter);
        assert!(res1.is_ok(), "First submit_data should succeed");

        // Second submission of same data should fail
        let res2 = contract.submit_data(1, valid_json).call(&submitter);
        assert!(res2.is_err(), "Second submit_data should fail on duplicate");
        let err = res2.unwrap_err().to_string();
        assert!(err.contains(&ContractError::DuplicateData.to_string()));
    }

    #[test]
    fn integration_submit_data_fails_on_invalid_json() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        let invalid_values = vec![
            serde_json::json!(""),
            serde_json::json!({}),
            serde_json::json!([]),
        ];

        for invalid_json in invalid_values {
            let res = contract
                .submit_data(1, invalid_json.clone())
                .call(&submitter);
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
    fn integration_submit_data_fails_on_inactive_sensor() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let user = "bob".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let sensor = json!({ "foo": "bar" });
        contract.submit_sensor(sensor).call(&user).unwrap();
        contract.activate(1).call(&admin).unwrap();
        contract.deactivate(1).call(&admin).unwrap();

        let data = json!({ "temperature": 42 });
        let res = contract.submit_data(1, data).call(&user);
        assert!(res.is_err(), "Submitting to inactive sensor should fail");
    }

    #[test]
    fn integration_add_verifier() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let non_admin = "intruder".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Add verifier
        let verifier = "verifier1".into_addr();
        let res = contract.add_verifier(verifier.clone()).call(&admin);
        assert!(res.is_ok(), "Admin should be able to add verifier");

        let is_verifier = contract.is_verifier(verifier.clone()).unwrap();
        assert!(is_verifier, "Verifier should be registered");

        // Try adding verifier as non_admin should fail
        let another_verifier = "verifier2".into_addr();
        let res = contract
            .add_verifier(another_verifier.clone())
            .call(&non_admin);
        assert!(res.is_err(), "Non-admin should NOT be able to add verifier");
        let err = res.unwrap_err().to_string();
        assert!(err.contains(&ContractError::Unauthorized.to_string()));
    }

    #[test]
    fn integration_verify_data_succeeds_for_registered_verifier() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        // Add verifier
        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();

        // Submit valid data by submitter
        let valid_json_str = r#"{"temperature": 22, "location": "field"}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
        contract
            .submit_data(1, valid_json)
            .call(&submitter)
            .unwrap();

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
        let admin = "admin".into_addr();
        let unregistered_verifier = "not_a_verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        // Submit valid data by submitter
        let valid_json_str = r#"{"temperature": 22, "location": "field"}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
        contract
            .submit_data(1, valid_json)
            .call(&submitter)
            .unwrap();

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
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        // Add verifier
        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();

        let valid_json_str = r#"{"temperature": 22, "location": "field"}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();
        contract
            .submit_data(1, valid_json)
            .call(&submitter)
            .unwrap();

        contract.verify_data(1).call(&verifier).unwrap();

        let res = contract.verify_data(1).call(&verifier);
        assert!(res.is_err(), "verify_data should fail if already verified");
        let err = res.unwrap_err().to_string();
        assert!(err.contains(&ContractError::AlreadyVerified.to_string()));
    }

    #[test]
    fn integration_reward_submitter_succeeds_for_verified_entry() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();
        let verifier = "verifier".into_addr();

        // Fund reward funds to admin so they can pay the reward
        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: admin.to_string(),
                amount: vec![coin(REWARD_AMOUNT, DEFAULT_DENOM)],
            }))
            .unwrap();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        // Add verifier
        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();

        let valid_json_str = r#"{"observation": "species A", "count": 5}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

        contract
            .submit_data(1, valid_json)
            .call(&submitter)
            .unwrap();

        contract.verify_data(1).call(&verifier).unwrap();

        let balance_before = ((*app.app())
            .wrap()
            .query::<BalanceResponse>(&QueryRequest::Bank(BankQuery::Balance {
                address: submitter.to_string(),
                denom: DEFAULT_DENOM.to_string(),
            }))
            .unwrap())
        .amount;

        // Now reward submitter by sending the reward funds with the call
        let res = contract
            .reward_submitter(1)
            .with_funds(&[coin(REWARD_AMOUNT, DEFAULT_DENOM)])
            .call(&admin);

        assert!(
            res.is_ok(),
            "reward_submitter should succeed for verified entry"
        );

        let entry = contract.get_data_entry(1).unwrap();
        assert!(entry.rewarded);

        assert!(
            res.unwrap().events.iter().any(|event| {
                event.ty == "wasm"
                    && event
                        .attributes
                        .iter()
                        .any(|attr| attr.key == "action" && attr.value == "reward_submitter")
            }),
            "Expected reward_submitter event"
        );

        let balance_after = ((*app.app())
            .wrap()
            .query::<BalanceResponse>(&QueryRequest::Bank(BankQuery::Balance {
                address: submitter.to_string(),
                denom: DEFAULT_DENOM.to_string(),
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
    fn integration_reward_submitter_fails_if_not_verified() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        let valid_json_str = r#"{"observation": "species B", "count": 3}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

        contract
            .submit_data(1, valid_json)
            .call(&submitter)
            .unwrap();

        // Attempt to reward submitter before verification
        let res = contract.reward_submitter(1).call(&admin);
        assert!(res.is_err(), "reward_submitter should fail if not verified");

        let err = res.unwrap_err().to_string();
        assert!(err.contains(&ContractError::NotVerified.to_string()));
    }

    #[test]
    fn integration_reward_submitter_fails_on_wrong_denom() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let submitter = "bob".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();

        contract
            .submit_sensor(json!({"name": "s"}))
            .call(&submitter)
            .unwrap();
        contract.activate(1).call(&admin).unwrap();

        contract
            .submit_data(1, json!({"val": 1}))
            .call(&submitter)
            .unwrap();
        contract.verify_data(1).call(&verifier).unwrap();

        // Try rewarding with wrong denom
        let res = contract
            .reward_submitter(1)
            .with_funds(&[coin(REWARD_AMOUNT, "uwrong")])
            .call(&admin);
        assert!(res.is_err());
    }

    #[test]
    fn integration_reward_submitter_fails_if_already_rewarded() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();
        let verifier = "verifier".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        // Add verifier
        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();

        let valid_json_str = r#"{"observation": "species C", "count": 1}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

        contract
            .submit_data(1, valid_json)
            .call(&submitter)
            .unwrap();
        contract.verify_data(1).call(&verifier).unwrap();

        // Fund reward 2 times funds to admin so they can pay the reward
        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: admin.to_string(),
                amount: vec![coin(REWARD_AMOUNT * 2, DEFAULT_DENOM)],
            }))
            .unwrap();

        // First reward call succeeds
        let _ = contract
            .reward_submitter(1)
            .with_funds(&[coin(REWARD_AMOUNT, DEFAULT_DENOM)])
            .call(&admin);

        // Second call should fail
        let res = contract
            .reward_submitter(1)
            .with_funds(&[coin(REWARD_AMOUNT, DEFAULT_DENOM)])
            .call(&admin);

        assert!(
            res.is_err(),
            "reward_submitter should fail if already rewarded"
        );

        let err = res.unwrap_err().to_string();

        assert!(err.contains(&ContractError::AlreadyRewarded.to_string()));
    }

    #[test]
    fn integration_get_sensor_returns_expected_sensor() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "activate should succeed");

        // Query the sensor
        let sensor = contract.get_sensor(1).unwrap();

        assert_eq!(sensor.id, 1);
        assert_eq!(sensor.owner, submitter);
        assert_eq!(sensor.data_str, sensor_json.to_string());
        assert_eq!(sensor.status.to_string(), "Active");
    }

    #[test]
    fn integration_get_data_entry_returns_correct_entry() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();
        let verifier = "verifier".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        // Add verifier
        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();

        let valid_json_str = r#"{"observation": "species A", "count": 5}"#.to_string();
        let valid_json: serde_json::Value = serde_json::from_str(&valid_json_str).unwrap();

        contract
            .submit_data(1, valid_json.clone())
            .call(&submitter)
            .unwrap();

        contract.verify_data(1).call(&verifier).unwrap();

        // Query the data entry
        let entry = contract.get_data_entry(1).unwrap();

        assert_eq!(entry.id, 1);
        assert_eq!(entry.data_str, valid_json.to_string());
        assert_eq!(entry.submitter, submitter);
        assert!(entry.verified);
        assert_eq!(entry.verifier, Some(verifier));
        assert!(!entry.rewarded);
    }

    #[test]
    fn integration_list_sensors_returns_expected_range() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit 3 sensors
        let sensor_jsons = vec![
            serde_json::json!({ "name": "sensor1", "location": "loc1" }),
            serde_json::json!({ "name": "sensor2", "location": "loc2" }),
            serde_json::json!({ "name": "sensor3", "location": "loc3" }),
        ];

        for json in &sensor_jsons {
            let res = contract.submit_sensor(json.clone()).call(&submitter);
            assert!(res.is_ok(), "submit_sensor should succeed");
        }

        // Query all sensors
        let sensors = contract.list_sensors(None, Some(10)).unwrap();
        assert_eq!(sensors.len(), 3);

        for (i, sensor) in sensors.iter().enumerate() {
            assert_eq!(sensor.id, (i + 1) as u64);
            assert_eq!(sensor.owner, submitter);
            assert_eq!(sensor.data_str, sensor_jsons[i].to_string());
        }

        // Paginated query: start_after = 1
        let page = contract.list_sensors(Some(1), Some(1)).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, 2);
        assert_eq!(page[0].data_str, sensor_jsons[1].to_string());
    }

    #[test]
    fn integration_list_sensors_returns_correct_range() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let sensor_jsons = vec![
            serde_json::json!({ "name": "sensor1", "location": "loc1" }),
            serde_json::json!({ "name": "sensor2", "location": "loc2" }),
            serde_json::json!({ "name": "sensor3", "location": "loc3" }),
        ];

        for json in &sensor_jsons {
            contract
                .submit_sensor(json.clone())
                .call(&submitter)
                .unwrap();
        }

        // Query all
        let sensors = contract.list_sensors(None, Some(10)).unwrap();
        assert_eq!(sensors.len(), 3);

        for (i, sensor) in sensors.iter().enumerate() {
            assert_eq!(sensor.id, (i + 1) as u64);
            assert_eq!(sensor.owner, submitter);
            assert_eq!(sensor.data_str, sensor_jsons[i].to_string());
        }

        // Paginated query: start_after = 1, limit = 1
        let page = contract.list_sensors(Some(1), Some(1)).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, 2);
        assert_eq!(page[0].data_str, sensor_jsons[1].to_string());
    }

    #[test]
    fn integration_list_sensors_pagination_edge_cases() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        for i in 0..5 {
            let sensor = json!({ "name": format!("Sensor{}", i), "location": format!("Loc{}", i) });
            contract.submit_sensor(sensor).call(&submitter).unwrap();
        }

        let result = contract.list_sensors(None, Some(0)).unwrap();
        assert!(result.is_empty());

        let result = contract.list_sensors(Some(5), Some(10)).unwrap();
        assert!(result.is_empty());

        // limit > total remaining should return all remaining
        let result = contract.list_sensors(Some(2), Some(10)).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, 3);
        assert_eq!(result[2].id, 5);
    }

    #[test]
    fn integration_list_data_entries_returns_correct_range() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        // Submit 3 entries
        let data_entries = vec![
            r#"{"species": "A", "count": 1}"#,
            r#"{"species": "B", "count": 2}"#,
            r#"{"species": "C", "count": 3}"#,
        ];

        for valid_json_str in &data_entries {
            let valid_json: serde_json::Value = serde_json::from_str(valid_json_str).unwrap();
            contract
                .submit_data(1, valid_json)
                .call(&submitter)
                .unwrap();
        }

        // Query all
        let entries = contract.list_data_entries(None, Some(10)).unwrap();
        assert_eq!(entries.len(), 3);
        for (i, entry) in entries.iter().enumerate() {
            let expected_data_str = serde_json::to_string(
                &serde_json::from_str::<serde_json::Value>(data_entries[i]).unwrap(),
            )
            .unwrap();

            assert_eq!(entry.id, (i + 1) as u64);
            assert_eq!(entry.data_str, expected_data_str);
            assert_eq!(entry.submitter, submitter);
        }

        // Query with pagination (start_after = 1)
        let entries_page = contract.list_data_entries(Some(1), Some(1)).unwrap();
        assert_eq!(entries_page.len(), 1);

        let expected_data_str = serde_json::to_string(
            &serde_json::from_str::<serde_json::Value>(data_entries[1]).unwrap(),
        )
        .unwrap();

        assert_eq!(entries_page[0].id, 2);
        assert_eq!(entries_page[0].data_str, expected_data_str);
    }

    #[test]
    fn integration_list_data_entries_pagination_edge_cases() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let submitter = "submitter".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&submitter);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        for i in 0..10 {
            let entry = json!({ "species": format!("Species{}", i), "count": i });
            contract
                .submit_data(1, entry.clone())
                .call(&submitter)
                .unwrap();
        }

        // limit = 0 should return 0
        let empty_page = contract.list_data_entries(None, Some(0)).unwrap();
        assert!(empty_page.is_empty());

        // start_after = last id should return 0
        let end_page = contract.list_data_entries(Some(10), Some(10)).unwrap();
        assert!(end_page.is_empty());

        // limit > total should return all remaining
        let page = contract.list_data_entries(Some(5), Some(10)).unwrap();
        assert_eq!(page.len(), 5);
    }

    #[test]
    fn integration_list_data_entries_respects_max_limit_30() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let user = "bob".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .submit_sensor(json!({ "foo": "bar" }))
            .call(&user)
            .unwrap();
        contract.activate(1).call(&admin).unwrap();

        for i in 0..35 {
            let entry = json!({ "data": i });
            contract.submit_data(1, entry).call(&user).unwrap();
        }

        let result = contract.list_data_entries(None, Some(30)).unwrap();
        assert_eq!(result.len(), 30);
    }

    #[test]
    fn integration_data_entries_track_multiple_submitters() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let alice = "alice".into_addr();
        let bob = "bob".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Submit a sensor (as submitter)
        let sensor_json = serde_json::json!({
            "name": "field",
            "location": "field"
        });
        let res = contract.submit_sensor(sensor_json.clone()).call(&alice);
        assert!(res.is_ok(), "submit_sensor should succeed");

        // Activate the sensor (as admin)
        let res = contract.activate(1).call(&admin);
        assert!(res.is_ok(), "sensor activation should succeed");

        let entry1 = json!({ "species": "A" });
        let entry2 = json!({ "species": "B" });

        contract
            .submit_data(1, entry1.clone())
            .call(&alice)
            .unwrap();
        contract.submit_data(1, entry2.clone()).call(&bob).unwrap();

        let entries = contract.list_data_entries(None, Some(10)).unwrap();
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].submitter, alice);
        assert_eq!(entries[1].submitter, bob);
    }

    #[test]
    fn integration_list_verifiers_returns_all_verifiers() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let verifier1 = "verifier1".into_addr();
        let verifier2 = "verifier2".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Add verifiers
        contract
            .add_verifier(verifier1.clone())
            .call(&admin)
            .unwrap();
        contract
            .add_verifier(verifier2.clone())
            .call(&admin)
            .unwrap();

        // Query list of verifiers
        let verifiers = contract.list_verifiers().unwrap();

        // Ensure both verifiers are returned
        assert_eq!(verifiers.len(), 2);
        assert!(verifiers.contains(&verifier1));
        assert!(verifiers.contains(&verifier2));
    }

    #[test]
    fn integration_list_verifiers_works() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let verifier1 = "ver1".into_addr();
        let verifier2 = "ver2".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let initial = contract.list_verifiers().unwrap();
        assert!(initial.is_empty());

        contract
            .add_verifier(verifier1.clone())
            .call(&admin)
            .unwrap();
        contract
            .add_verifier(verifier2.clone())
            .call(&admin)
            .unwrap();

        let verifiers = contract.list_verifiers().unwrap();
        assert_eq!(verifiers.len(), 2);
        assert!(verifiers.contains(&verifier1));
        assert!(verifiers.contains(&verifier2));
    }

    #[test]
    fn integration_is_verifier_returns_correct_status() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);

        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let non_verifier = "someone_else".into_addr();

        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        // Initially, no one is a verifier
        let res = contract.is_verifier(verifier.clone()).unwrap();
        assert!(!res, "verifier should not be listed initially");

        // Add a verifier
        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();

        let res = contract.is_verifier(verifier.clone()).unwrap();
        assert!(res, "verifier should be listed after being added");

        let res = contract.is_verifier(non_verifier.clone()).unwrap();
        assert!(!res, "non-verifier should not be listed");
    }

    #[test]
    fn integration_is_verifier_returns_false_for_unknown() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let unknown = "ghost".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let res = contract.is_verifier(unknown.clone()).unwrap();
        assert!(!res, "Unknown address should not be a verifier");
    }
}
