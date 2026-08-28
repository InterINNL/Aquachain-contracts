#[cfg(test)]
mod tests {
    use crate::actions::{ACTION_MINT_CREDITS, ACTION_POST_BOUNTY};
    use crate::contract::LocalDaoContract;
    use crate::contract::sv::mt::CodeId;
    use crate::contract::sv::mt::LocalDaoContractProxy;
    use crate::contract::{InstantiateConfig, ProposalStatus, VoteOption};
    use crate::errors::ContractError;
    use community_bounty::contract::BountyStatus;
    use community_bounty::contract::CommunityBountyContract;
    use community_bounty::contract::sv::mt::CodeId as BountyCodeId;
    use community_bounty::contract::sv::mt::CommunityBountyContractProxy;
    use serde_json::json;
    use sylvia::cw_multi_test::{App as MtApp, BankSudo, IntoAddr, SudoMsg};
    use sylvia::cw_std::{Timestamp, Uint128, coin};
    use sylvia::multitest::App;
    use sylvia::multitest::Proxy;
    use water_credit_marketplace::contract::WaterCreditMarketplaceContract;
    use water_credit_marketplace::contract::sv::mt::CodeId as WcmCodeId;
    use water_credit_marketplace::contract::sv::mt::WaterCreditMarketplaceContractProxy;

    const TEST_DENOM: &str = "ustake";

    fn mint(app: &App<MtApp>, to: impl Into<String>, amount: u128) {
        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: to.into(),
                amount: vec![coin(amount, TEST_DENOM)],
            }))
            .unwrap();
    }

    fn advance_time(app: &App<MtApp>, seconds: u64) {
        app.app_mut().update_block(|block| {
            block.time = Timestamp::from_seconds(block.time.seconds() + seconds);
        });
    }

    fn bounty_metadata(city: &str, deadline: u64, reward: &str) -> serde_json::Value {
        json!({
            "location": city,
            "summary": "Community sustainability decision",
            "deadline": deadline,
            "reward": reward,
        })
    }

    fn mint_metadata(recipient: &str, amount: &str) -> serde_json::Value {
        json!({
            "location": "Delhi, India",
            "summary": "Mint water credits for verified cleanup",
            "recipient": recipient,
            "amount": amount,
        })
    }

    fn dao_config(
        quorum_bps: u64,
        community_bounty: Option<sylvia::cw_std::Addr>,
    ) -> InstantiateConfig {
        InstantiateConfig {
            quorum_bps: Some(quorum_bps),
            voting_period_seconds: Some(100),
            community_bounty,
            water_credit_marketplace: None,
            citizen_science_registry: None,
            default_denom: Some(TEST_DENOM.to_string()),
        }
    }

    fn setup_dao_with_bounty_quorum(
        app: &App<MtApp>,
        quorum_bps: u64,
    ) -> (
        Proxy<'_, MtApp, LocalDaoContract>,
        Proxy<'_, MtApp, CommunityBountyContract>,
    ) {
        let deployer = "deployer".into_addr();
        let bounty_code = BountyCodeId::store_code(app);
        let bounty = bounty_code
            .instantiate(Some(TEST_DENOM.to_string()))
            .call(&deployer)
            .unwrap();

        let dao_code = CodeId::store_code(app);
        let dao = dao_code
            .instantiate(dao_config(quorum_bps, Some(bounty.contract_addr.clone())))
            .call(&deployer)
            .unwrap();

        (dao, bounty)
    }

    fn setup_dao_with_bounty(
        app: &App<MtApp>,
    ) -> (
        Proxy<'_, MtApp, LocalDaoContract>,
        Proxy<'_, MtApp, CommunityBountyContract>,
    ) {
        setup_dao_with_bounty_quorum(app, 3000)
    }

    fn setup_dao_with_wcm(
        app: &App<MtApp>,
    ) -> (
        Proxy<'_, MtApp, LocalDaoContract>,
        Proxy<'_, MtApp, WaterCreditMarketplaceContract>,
    ) {
        let deployer = "deployer".into_addr();
        let dao_code = CodeId::store_code(app);
        let dao = dao_code
            .instantiate(dao_config(3000, None))
            .call(&deployer)
            .unwrap();

        let wcm_code = WcmCodeId::store_code(app);
        let wcm = wcm_code
            .instantiate(Some(TEST_DENOM.to_string()))
            .call(&dao.contract_addr)
            .unwrap();

        dao.update_action_targets(None, Some(wcm.contract_addr.clone()), None)
            .call(&deployer)
            .unwrap();

        (dao, wcm)
    }

    #[test]
    fn create_proposal_and_vote_once() {
        let app = App::default();
        let (contract, _) = setup_dao_with_bounty(&app);
        let proposer = "proposer".into_addr();
        let voter = "voter".into_addr();
        let deadline = app.app().block_info().time.seconds() + 3600;

        contract
            .create_proposal(
                "Yamuna cleanup budget — Okhla".to_string(),
                "Allocate volunteer supplies for a Delhi riverbank cleanup.".to_string(),
                ACTION_POST_BOUNTY.to_string(),
                bounty_metadata("Delhi, India", deadline, "500"),
            )
            .call(&proposer)
            .unwrap();

        contract.vote(1, VoteOption::Yes).call(&voter).unwrap();

        let vote = contract.get_vote(1, voter.clone()).unwrap();
        assert!(matches!(vote.vote, VoteOption::Yes));

        let dup = contract.vote(1, VoteOption::No).call(&voter);
        assert!(dup.is_err());
        assert!(
            dup.unwrap_err()
                .to_string()
                .contains(&ContractError::AlreadyVoted.to_string())
        );
    }

    #[test]
    fn rejects_unsupported_action_tag() {
        let app = App::default();
        let (contract, _) = setup_dao_with_bounty(&app);
        let proposer = "proposer".into_addr();
        let deadline = app.app().block_info().time.seconds() + 3600;

        let err = contract
            .create_proposal(
                "Legacy tag".to_string(),
                "Should fail".to_string(),
                "fund_cleanup".to_string(),
                bounty_metadata("Delhi, India", deadline, "500"),
            )
            .call(&proposer);
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains(&ContractError::UnsupportedAction.to_string())
        );
    }

    #[test]
    fn open_proposal_cannot_execute() {
        let app = App::default();
        let (contract, _) = setup_dao_with_bounty(&app);
        let proposer = "proposer".into_addr();
        let deadline = app.app().block_info().time.seconds() + 3600;

        contract
            .create_proposal(
                "Yamuna cleanup budget — Okhla".to_string(),
                "Allocate volunteer supplies for a Delhi riverbank cleanup.".to_string(),
                ACTION_POST_BOUNTY.to_string(),
                bounty_metadata("Delhi, India", deadline, "500"),
            )
            .call(&proposer)
            .unwrap();

        let err = contract.execute_proposal(1).call(&proposer);
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains(&ContractError::VotingNotEnded.to_string())
        );
    }

    #[test]
    fn quorum_enforcement() {
        let app = App::default();
        let (contract, _) = setup_dao_with_bounty_quorum(&app, 5000);
        let proposer = "proposer".into_addr();
        let voter_a = "voter_a".into_addr();
        let voter_b = "voter_b".into_addr();
        let voter_c = "voter_c".into_addr();
        let deadline = app.app().block_info().time.seconds() + 3600;

        contract
            .create_proposal(
                "Yamuna cleanup budget — Okhla".to_string(),
                "Allocate volunteer supplies for a Delhi riverbank cleanup.".to_string(),
                ACTION_POST_BOUNTY.to_string(),
                bounty_metadata("Delhi, India", deadline, "500"),
            )
            .call(&proposer)
            .unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_a).unwrap();

        contract
            .create_proposal(
                "Register voters".to_string(),
                "Bootstrap voter registry".to_string(),
                ACTION_POST_BOUNTY.to_string(),
                bounty_metadata("Bengaluru, India", deadline, "100"),
            )
            .call(&proposer)
            .unwrap();
        contract.vote(2, VoteOption::Yes).call(&voter_b).unwrap();
        contract.vote(2, VoteOption::Yes).call(&voter_c).unwrap();

        advance_time(&app, 200);

        contract.execute_proposal(1).call(&proposer).unwrap();

        let proposal = contract.get_proposal(1).unwrap();
        assert!(matches!(proposal.status, ProposalStatus::Failed));
    }

    #[test]
    fn passed_post_bounty_executes_sibling_contract() {
        let app = App::default();
        let (contract, bounty) = setup_dao_with_bounty(&app);
        let proposer = "proposer".into_addr();
        let voter_a = "voter_a".into_addr();
        let voter_b = "voter_b".into_addr();
        let executor = "executor".into_addr();
        let deadline = app.app().block_info().time.seconds() + 3600;

        contract
            .create_proposal(
                "Yamuna cleanup crew — Okhla".to_string(),
                "Remove plastic from a 500 m riverbank stretch".to_string(),
                ACTION_POST_BOUNTY.to_string(),
                bounty_metadata("Delhi, India", deadline, "500"),
            )
            .call(&proposer)
            .unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_a).unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_b).unwrap();

        advance_time(&app, 200);
        mint(&app, &executor, 500);

        contract
            .execute_proposal(1)
            .with_funds(&[coin(500, TEST_DENOM)])
            .call(&executor)
            .unwrap();

        let proposal = contract.get_proposal(1).unwrap();
        assert!(matches!(proposal.status, ProposalStatus::Executed));

        let listed = bounty.list_bounties(None, None).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].reward_amount, Uint128::new(500));
        assert!(matches!(listed[0].status, BountyStatus::Open));
    }

    #[test]
    fn passed_mint_credits_executes_sibling_contract() {
        let app = App::default();
        let (contract, wcm) = setup_dao_with_wcm(&app);
        let proposer = "proposer".into_addr();
        let voter_a = "voter_a".into_addr();
        let voter_b = "voter_b".into_addr();
        let beneficiary = "beneficiary".into_addr();

        contract
            .create_proposal(
                "Mint cleanup credits".to_string(),
                "Reward verified river stewards with tradable credits.".to_string(),
                ACTION_MINT_CREDITS.to_string(),
                mint_metadata(&beneficiary.to_string(), "75"),
            )
            .call(&proposer)
            .unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_a).unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_b).unwrap();

        advance_time(&app, 200);

        contract.execute_proposal(1).call(&proposer).unwrap();

        let proposal = contract.get_proposal(1).unwrap();
        assert!(matches!(proposal.status, ProposalStatus::Executed));

        let balance = wcm.get_balance(beneficiary.clone()).unwrap();
        assert_eq!(balance, Uint128::new(75));
    }

    #[test]
    fn execute_requires_matching_reward_funds() {
        let app = App::default();
        let (contract, _) = setup_dao_with_bounty(&app);
        let proposer = "proposer".into_addr();
        let voter_a = "voter_a".into_addr();
        let voter_b = "voter_b".into_addr();
        let deadline = app.app().block_info().time.seconds() + 3600;

        contract
            .create_proposal(
                "Yamuna cleanup crew — Okhla".to_string(),
                "Remove plastic from a 500 m riverbank stretch".to_string(),
                ACTION_POST_BOUNTY.to_string(),
                bounty_metadata("Delhi, India", deadline, "500"),
            )
            .call(&proposer)
            .unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_a).unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_b).unwrap();

        advance_time(&app, 200);
        mint(&app, &proposer, 500);

        let err = contract
            .execute_proposal(1)
            .with_funds(&[coin(400, TEST_DENOM)])
            .call(&proposer);
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains(&ContractError::InvalidFundsAmount.to_string())
        );
    }

    #[test]
    fn successful_execute_runs_once() {
        let app = App::default();
        let (contract, _) = setup_dao_with_bounty(&app);
        let proposer = "proposer".into_addr();
        let voter_a = "voter_a".into_addr();
        let voter_b = "voter_b".into_addr();
        let deadline = app.app().block_info().time.seconds() + 3600;

        contract
            .create_proposal(
                "Yamuna cleanup budget — Okhla".to_string(),
                "Allocate volunteer supplies for a Delhi riverbank cleanup.".to_string(),
                ACTION_POST_BOUNTY.to_string(),
                bounty_metadata("Delhi, India", deadline, "500"),
            )
            .call(&proposer)
            .unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_a).unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_b).unwrap();

        advance_time(&app, 200);
        mint(&app, &proposer, 500);

        contract
            .execute_proposal(1)
            .with_funds(&[coin(500, TEST_DENOM)])
            .call(&proposer)
            .unwrap();

        let proposal = contract.get_proposal(1).unwrap();
        assert!(matches!(proposal.status, ProposalStatus::Executed));

        let again = contract.execute_proposal(1).call(&proposer);
        assert!(again.is_err());
        assert!(again.unwrap_err().to_string().contains("finalized"));
    }

    #[test]
    fn failed_proposal_cannot_execute_successfully() {
        let app = App::default();
        let (contract, _) = setup_dao_with_bounty(&app);
        let proposer = "proposer".into_addr();
        let voter_a = "voter_a".into_addr();
        let voter_b = "voter_b".into_addr();
        let deadline = app.app().block_info().time.seconds() + 3600;

        contract
            .create_proposal(
                "Yamuna cleanup budget — Okhla".to_string(),
                "Allocate volunteer supplies for a Delhi riverbank cleanup.".to_string(),
                ACTION_POST_BOUNTY.to_string(),
                bounty_metadata("Delhi, India", deadline, "500"),
            )
            .call(&proposer)
            .unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_a).unwrap();
        contract.vote(1, VoteOption::No).call(&voter_b).unwrap();

        advance_time(&app, 200);

        contract.execute_proposal(1).call(&proposer).unwrap();

        let proposal = contract.get_proposal(1).unwrap();
        assert!(matches!(proposal.status, ProposalStatus::Failed));
    }

    #[test]
    fn list_proposals_returns_created_rows() {
        let app = App::default();
        let (contract, _) = setup_dao_with_bounty(&app);
        let proposer = "proposer".into_addr();
        let deadline = app.app().block_info().time.seconds() + 3600;

        contract
            .create_proposal(
                "Yamuna cleanup budget — Okhla".to_string(),
                "Allocate volunteer supplies for a Delhi riverbank cleanup.".to_string(),
                ACTION_POST_BOUNTY.to_string(),
                bounty_metadata("Delhi, India", deadline, "500"),
            )
            .call(&proposer)
            .unwrap();
        contract
            .create_proposal(
                "Lake Pichola tree planting".to_string(),
                "Approve sapling budget for Udaipur shoreline.".to_string(),
                ACTION_POST_BOUNTY.to_string(),
                bounty_metadata("Udaipur, India", deadline, "200"),
            )
            .call(&proposer)
            .unwrap();

        let listed = contract.list_proposals(None, None).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[1].title, "Lake Pichola tree planting");
    }
}
