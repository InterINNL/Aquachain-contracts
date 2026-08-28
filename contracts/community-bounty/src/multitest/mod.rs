#[cfg(test)]
mod tests {
    use crate::constants::DEFAULT_DENOM;
    use crate::contract::sv::mt::CodeId;
    use crate::contract::sv::mt::CommunityBountyContractProxy;
    use crate::contract::{BountyStatus, WorkSubmission};
    use crate::errors::ContractError;
    use serde_json::json;
    use sylvia::cw_multi_test::{App as MtApp, BankSudo, IntoAddr, SudoMsg};
    use sylvia::cw_std::{Timestamp, Uint128, coin};
    use sylvia::multitest::App;

    fn mint(app: &App<MtApp>, to: impl Into<String>, amount: u128) {
        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: to.into(),
                amount: vec![coin(amount, DEFAULT_DENOM)],
            }))
            .unwrap();
    }

    fn sample_work(summary: &str) -> serde_json::Value {
        json!({
            "summary": summary,
            "location": "Delhi, India",
            "evidence": "Photo set and weigh-in log attached",
            "hours_spent": "6"
        })
    }

    fn advance_time(app: &App<MtApp>, seconds: u64) {
        app.app_mut().update_block(|block| {
            block.time = Timestamp::from_seconds(block.time.seconds() + seconds);
        });
    }

    #[test]
    fn post_bounty_and_list() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let poster = "poster".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&poster)
            .unwrap();

        mint(&app, &poster, 500);
        contract
            .post_bounty(
                "Yamuna cleanup crew — Okhla".to_string(),
                "Remove plastic from a 500 m riverbank stretch".to_string(),
                "Delhi, India".to_string(),
                9_999_999_999,
            )
            .with_funds(&[coin(500, DEFAULT_DENOM)])
            .call(&poster)
            .unwrap();

        let listed = contract.list_bounties(None, None).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, 1);
        assert_eq!(listed[0].poster, poster);
        assert_eq!(listed[0].reward_amount, Uint128::new(500));
        assert!(matches!(listed[0].status, BountyStatus::Open));
    }

    #[test]
    fn reject_submit_after_deadline() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let poster = "poster".into_addr();
        let worker = "worker".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&poster)
            .unwrap();

        mint(&app, &poster, 300);
        let deadline = app.app().block_info().time.seconds() + 100;
        contract
            .post_bounty(
                "Lake Pichola litter pick".to_string(),
                "Collect and sort waste along the lake path".to_string(),
                "Udaipur, India".to_string(),
                deadline,
            )
            .with_funds(&[coin(300, DEFAULT_DENOM)])
            .call(&poster)
            .unwrap();

        advance_time(&app, 200);

        let late = contract
            .submit_work(1, sample_work("Completed sweep"))
            .call(&worker);
        assert!(late.is_err());
        assert!(
            late.unwrap_err()
                .to_string()
                .contains(&ContractError::DeadlinePassed.to_string())
        );
    }

    #[test]
    fn poster_only_approve() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let poster = "poster".into_addr();
        let worker = "worker".into_addr();
        let stranger = "stranger".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&poster)
            .unwrap();

        mint(&app, &poster, 800);
        contract
            .post_bounty(
                "Juhu beach plastic sweep".to_string(),
                "Morning volunteer shift with Mumbai NGO".to_string(),
                "Mumbai, India".to_string(),
                9_999_999_999,
            )
            .with_funds(&[coin(800, DEFAULT_DENOM)])
            .call(&poster)
            .unwrap();

        contract
            .submit_work(1, sample_work("Collected 42 kg plastic"))
            .call(&worker)
            .unwrap();

        let bad = contract.approve_work(1, 1).call(&stranger);
        assert!(bad.is_err());
        assert!(
            bad.unwrap_err()
                .to_string()
                .contains(&ContractError::Unauthorized.to_string())
        );

        contract.approve_work(1, 1).call(&poster).unwrap();

        let bounty = contract.get_bounty(1).unwrap();
        assert!(matches!(bounty.status, BountyStatus::Completed));
        assert_eq!(bounty.winner, Some(worker));
    }

    #[test]
    fn payout_equals_escrowed_reward() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let poster = "poster".into_addr();
        let worker = "worker".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&poster)
            .unwrap();

        let reward = 1_250u128;
        mint(&app, &poster, reward);
        contract
            .post_bounty(
                "Bengaluru lake desilting".to_string(),
                "Remove silt from stormwater inlet".to_string(),
                "Bengaluru, India".to_string(),
                9_999_999_999,
            )
            .with_funds(&[coin(reward, DEFAULT_DENOM)])
            .call(&poster)
            .unwrap();

        contract
            .submit_work(1, sample_work("Desilting complete with photos"))
            .call(&worker)
            .unwrap();

        contract.approve_work(1, 1).call(&poster).unwrap();

        let balance = app
            .app()
            .wrap()
            .query_balance(worker.clone(), DEFAULT_DENOM)
            .unwrap();
        assert_eq!(balance.amount, Uint128::new(reward));
    }

    #[test]
    fn cancel_returns_escrow_to_poster() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let poster = "poster".into_addr();
        let stranger = "stranger".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&poster)
            .unwrap();

        let reward = 600u128;
        mint(&app, &poster, reward);
        contract
            .post_bounty(
                "Nashik rainwater harvesting install".to_string(),
                "Help install rooftop collection barrels".to_string(),
                "Nashik, India".to_string(),
                9_999_999_999,
            )
            .with_funds(&[coin(reward, DEFAULT_DENOM)])
            .call(&poster)
            .unwrap();

        let bad = contract.cancel_bounty(1).call(&stranger);
        assert!(bad.is_err());

        contract.cancel_bounty(1).call(&poster).unwrap();

        let bounty = contract.get_bounty(1).unwrap();
        assert!(matches!(bounty.status, BountyStatus::Cancelled));

        let balance = app
            .app()
            .wrap()
            .query_balance(poster.clone(), DEFAULT_DENOM)
            .unwrap();
        assert_eq!(balance.amount, Uint128::new(reward));
    }

    #[test]
    fn cannot_cancel_completed_bounty() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let poster = "poster".into_addr();
        let worker = "worker".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&poster)
            .unwrap();

        mint(&app, &poster, 400);
        contract
            .post_bounty(
                "Gujarat mangrove planting".to_string(),
                "Plant 200 saplings along tidal creek".to_string(),
                "Gujarat, India".to_string(),
                9_999_999_999,
            )
            .with_funds(&[coin(400, DEFAULT_DENOM)])
            .call(&poster)
            .unwrap();

        contract
            .submit_work(1, sample_work("All saplings planted"))
            .call(&worker)
            .unwrap();
        contract.approve_work(1, 1).call(&poster).unwrap();

        let err = contract.cancel_bounty(1).call(&poster);
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains(&ContractError::CannotCancel.to_string())
        );
    }

    #[test]
    fn list_submissions_filters_by_bounty() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let poster = "poster".into_addr();
        let worker_a = "worker_a".into_addr();
        let worker_b = "worker_b".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&poster)
            .unwrap();

        mint(&app, &poster, 200);
        for (title, loc) in [("Task A", "Delhi, India"), ("Task B", "Chennai, India")] {
            contract
                .post_bounty(
                    title.to_string(),
                    "Demo bounty".to_string(),
                    loc.to_string(),
                    9_999_999_999,
                )
                .with_funds(&[coin(100, DEFAULT_DENOM)])
                .call(&poster)
                .unwrap();
        }

        contract
            .submit_work(1, sample_work("Work on bounty 1"))
            .call(&worker_a)
            .unwrap();
        contract
            .submit_work(2, sample_work("Work on bounty 2"))
            .call(&worker_b)
            .unwrap();

        let all: Vec<WorkSubmission> = contract.list_submissions(None, None, None).unwrap();
        assert_eq!(all.len(), 2);

        let for_one = contract.list_submissions(Some(1), None, None).unwrap();
        assert_eq!(for_one.len(), 1);
        assert_eq!(for_one[0].bounty_id, 1);
    }
}
