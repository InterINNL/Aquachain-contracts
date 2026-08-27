#[cfg(test)]
mod tests {
    use crate::constants::DEFAULT_DENOM;
    use crate::contract::sv::mt::CodeId;
    use crate::contract::sv::mt::UtilityWaterFootprintContractProxy;
    use crate::errors::ContractError;
    use serde_json::json;
    use sylvia::cw_multi_test::IntoAddr;
    use sylvia::cw_std::Uint128;
    use sylvia::multitest::App;

    #[test]
    fn register_company_and_get_profile() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        let metadata = json!({ "sector": "utility", "region": "north" });

        contract
            .register_company("Aqua Co".to_string(), metadata.clone())
            .call(&owner)
            .unwrap();

        let company = contract.get_company(1).unwrap();
        assert_eq!(company.id, 1);
        assert_eq!(company.owner, owner);
        assert_eq!(company.name, "Aqua Co");
        assert_eq!(company.metadata_str, metadata.to_string());
    }

    #[test]
    fn log_usage_rejects_unauthenticated_caller() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let stranger = "stranger".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .register_company("Aqua Co".to_string(), json!({"ok": true}))
            .call(&owner)
            .unwrap();

        let res = contract
            .log_usage(
                1,
                "2026-Q1".to_string(),
                Uint128::new(1000),
                Uint128::new(100),
            )
            .call(&stranger);
        assert!(res.is_err(), "non-owner must not log usage");
        assert!(
            res.unwrap_err()
                .to_string()
                .contains(&ContractError::Unauthorized.to_string())
        );
    }

    #[test]
    fn log_usage_rejects_illogical_metrics_and_empty_period() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .register_company("Aqua Co".to_string(), json!({"ok": true}))
            .call(&owner)
            .unwrap();

        let empty_period = contract
            .log_usage(1, "  ".to_string(), Uint128::new(100), Uint128::new(10))
            .call(&owner);
        assert!(empty_period.is_err());
        assert!(
            empty_period
                .unwrap_err()
                .to_string()
                .contains(&ContractError::EmptyPeriod.to_string())
        );

        let illogical = contract
            .log_usage(
                1,
                "2026-Q1".to_string(),
                Uint128::new(100),
                Uint128::new(150),
            )
            .call(&owner);
        assert!(illogical.is_err());
        assert!(
            illogical
                .unwrap_err()
                .to_string()
                .contains(&ContractError::IllogicalMetrics.to_string())
        );

        let zero_usage = contract
            .log_usage(1, "2026-Q1".to_string(), Uint128::zero(), Uint128::zero())
            .call(&owner);
        assert!(zero_usage.is_err());
        assert!(
            zero_usage
                .unwrap_err()
                .to_string()
                .contains(&ContractError::ZeroUsage.to_string())
        );
    }

    #[test]
    fn only_verifier_or_admin_can_validate_log() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let verifier = "verifier".into_addr();
        let outsider = "outsider".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .register_company("Aqua Co".to_string(), json!({"ok": true}))
            .call(&owner)
            .unwrap();
        contract
            .log_usage(
                1,
                "2026-Q1".to_string(),
                Uint128::new(1000),
                Uint128::new(200),
            )
            .call(&owner)
            .unwrap();

        let denied = contract.validate_log(1).call(&outsider);
        assert!(denied.is_err());

        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();
        contract.validate_log(1).call(&verifier).unwrap();

        let log = contract.list_logs(Some(1), Some(10), None).unwrap();
        assert_eq!(log.len(), 1);
        assert!(log[0].validated);
        assert_eq!(log[0].validator.as_ref(), Some(&verifier));
    }

    #[test]
    fn certificate_requires_criteria_and_blocks_double_issue() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .register_company("Aqua Co".to_string(), json!({"ok": true}))
            .call(&owner)
            .unwrap();

        // 5% savings: below 10% threshold
        contract
            .log_usage(
                1,
                "2026-Q1".to_string(),
                Uint128::new(1000),
                Uint128::new(50),
            )
            .call(&owner)
            .unwrap();
        contract.validate_log(1).call(&admin).unwrap();

        let low = contract
            .issue_certificate(1, "2026-Q1".to_string())
            .call(&admin);
        assert!(low.is_err());
        assert!(
            low.unwrap_err()
                .to_string()
                .contains(&ContractError::CriteriaNotMet.to_string())
        );

        // Add another validated log bringing ratio to 15%
        contract
            .log_usage(
                1,
                "2026-Q1".to_string(),
                Uint128::new(1000),
                Uint128::new(250),
            )
            .call(&owner)
            .unwrap();
        contract.validate_log(2).call(&admin).unwrap();

        contract
            .issue_certificate(1, "2026-Q1".to_string())
            .call(&admin)
            .unwrap();

        let cert = contract.get_certificate(1).unwrap();
        assert_eq!(cert.company_id, 1);
        assert_eq!(cert.period, "2026-Q1");
        assert_eq!(cert.total_usage, Uint128::new(2000));
        assert_eq!(cert.total_savings, Uint128::new(300));

        let again = contract
            .issue_certificate(1, "2026-Q1".to_string())
            .call(&admin);
        assert!(again.is_err());
        assert!(
            again
                .unwrap_err()
                .to_string()
                .contains(&ContractError::AlreadyIssued.to_string())
        );
    }

    #[test]
    fn list_companies_and_certificates_paginate() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        for i in 1..=3 {
            contract
                .register_company(format!("Co {i}"), json!({ "n": i }))
                .call(&owner)
                .unwrap();
        }

        let page1 = contract.list_companies(Some(2), None).unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].id, 1);
        assert_eq!(page1[1].id, 2);

        let page2 = contract.list_companies(Some(2), Some(2)).unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].id, 3);

        contract
            .log_usage(
                1,
                "2026-Q2".to_string(),
                Uint128::new(100),
                Uint128::new(20),
            )
            .call(&owner)
            .unwrap();
        contract.validate_log(1).call(&admin).unwrap();
        contract
            .issue_certificate(1, "2026-Q2".to_string())
            .call(&admin)
            .unwrap();

        let certs = contract.list_certificates(Some(1), Some(10), None).unwrap();
        assert_eq!(certs.len(), 1);
        assert_eq!(certs[0].id, 1);
    }

    #[test]
    fn remove_verifier_is_admin_only() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let verifier = "verifier".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();
        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();
        assert!(contract.is_verifier(verifier.clone()).unwrap());

        let denied = contract.remove_verifier(verifier.clone()).call(&owner);
        assert!(denied.is_err());

        contract
            .remove_verifier(verifier.clone())
            .call(&admin)
            .unwrap();
        assert!(!contract.is_verifier(verifier).unwrap());
    }
}
