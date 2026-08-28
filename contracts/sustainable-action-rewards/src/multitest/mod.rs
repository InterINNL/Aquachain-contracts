#[cfg(test)]
mod tests {
    use crate::constants::DEFAULT_DENOM;
    use crate::contract::sv::mt::CodeId;
    use crate::contract::sv::mt::SustainableActionRewardsContractProxy;
    use crate::errors::ContractError;
    use serde_json::json;
    use sylvia::cw_multi_test::{BankSudo, IntoAddr, SudoMsg};
    use sylvia::cw_std::{Uint128, coin};
    use sylvia::multitest::App;

    fn sample_evidence(title: &str, points: &str) -> serde_json::Value {
        json!({
            "title": title,
            "location": "Delhi, India",
            "description": "Riverbank cleanup with photo evidence",
            "impact_points": points
        })
    }

    #[test]
    fn submit_action_and_list() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let actor = "actor".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .submit_action(sample_evidence("Yamuna cleanup — Okhla", "100"))
            .call(&actor)
            .unwrap();

        let listed = contract.list_actions(None, None).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, 1);
        assert_eq!(listed[0].actor, actor);
        assert_eq!(listed[0].impact_points, Uint128::new(100));
        assert!(!listed[0].verified);
    }

    #[test]
    fn duplicate_evidence_rejected() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let actor = "actor".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let evidence = sample_evidence("Lake Pichola litter pick — Udaipur", "50");
        contract
            .submit_action(evidence.clone())
            .call(&actor)
            .unwrap();

        let dup = contract.submit_action(evidence).call(&actor);
        assert!(dup.is_err());
        assert!(
            dup.unwrap_err()
                .to_string()
                .contains(&ContractError::DuplicateEvidence.to_string())
        );
    }

    #[test]
    fn verify_requires_verifier_or_admin() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let actor = "actor".into_addr();
        let stranger = "stranger".into_addr();
        let verifier = "verifier".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .submit_action(sample_evidence("Mumbai beach cleanup", "75"))
            .call(&actor)
            .unwrap();

        let bad = contract.verify_action(1).call(&stranger);
        assert!(bad.is_err());

        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();
        contract.verify_action(1).call(&verifier).unwrap();

        let eco = contract.get_action(1).unwrap();
        assert!(eco.verified);
    }

    #[test]
    fn impact_totals_after_verify() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let actor = "actor".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .submit_action(sample_evidence("Nashik tree planting drive", "40"))
            .call(&actor)
            .unwrap();
        contract
            .submit_action(sample_evidence("Bengaluru lake restoration", "60"))
            .call(&actor)
            .unwrap();

        contract.verify_action(1).call(&admin).unwrap();
        contract.verify_action(2).call(&admin).unwrap();

        let impact = contract.get_actor_impact(actor).unwrap();
        assert_eq!(impact, Uint128::new(100));
    }

    #[test]
    fn no_double_reward() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let actor = "actor".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: admin.to_string(),
                amount: vec![coin(5000, DEFAULT_DENOM)],
            }))
            .unwrap();

        contract
            .submit_action(sample_evidence("Gujarat rainwater harvesting demo", "30"))
            .call(&actor)
            .unwrap();
        contract.verify_action(1).call(&admin).unwrap();

        contract
            .reward_actor(1)
            .with_funds(&[coin(1000, DEFAULT_DENOM)])
            .call(&admin)
            .unwrap();

        let again = contract
            .reward_actor(1)
            .with_funds(&[coin(1000, DEFAULT_DENOM)])
            .call(&admin);
        assert!(again.is_err());
        assert!(
            again
                .unwrap_err()
                .to_string()
                .contains(&ContractError::AlreadyRewarded.to_string())
        );
    }

    #[test]
    fn remove_verifier_is_admin_only() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let verifier = "verifier".into_addr();
        let stranger = "stranger".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .add_verifier(verifier.clone())
            .call(&admin)
            .unwrap();

        let bad = contract.remove_verifier(verifier.clone()).call(&stranger);
        assert!(bad.is_err());

        contract
            .remove_verifier(verifier.clone())
            .call(&admin)
            .unwrap();
        assert!(!contract.is_verifier(verifier).unwrap());
    }
}
