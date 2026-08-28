#[cfg(test)]
mod tests {
    use crate::contract::sv::mt::CodeId;
    use crate::contract::sv::mt::LocalDaoContractProxy;
    use crate::contract::{ProposalStatus, VoteOption};
    use crate::errors::ContractError;
    use serde_json::json;
    use sylvia::cw_multi_test::App as MtApp;
    use sylvia::cw_multi_test::IntoAddr;
    use sylvia::cw_std::Timestamp;
    use sylvia::multitest::App;

    fn sample_metadata(city: &str) -> serde_json::Value {
        json!({
            "location": city,
            "summary": "Community sustainability decision",
            "language": "en"
        })
    }

    fn sample_proposal_msgs(city: &str) -> (String, String, String, serde_json::Value) {
        (
            "Yamuna cleanup budget — Okhla".to_string(),
            "Allocate volunteer supplies for a Delhi riverbank cleanup.".to_string(),
            "fund_cleanup".to_string(),
            sample_metadata(city),
        )
    }

    fn advance_time(app: &App<MtApp>, seconds: u64) {
        app.app_mut().update_block(|block| {
            block.time = Timestamp::from_seconds(block.time.seconds() + seconds);
        });
    }

    #[test]
    fn create_proposal_and_vote_once() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let proposer = "proposer".into_addr();
        let voter = "voter".into_addr();
        let contract = code_id
            .instantiate(Some(3000), Some(3600))
            .call(&proposer)
            .unwrap();

        let (title, desc, tag, meta) = sample_proposal_msgs("Delhi, India");
        contract
            .create_proposal(title, desc, tag, meta)
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
    fn open_proposal_cannot_execute() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let proposer = "proposer".into_addr();
        let contract = code_id
            .instantiate(Some(3000), Some(3600))
            .call(&proposer)
            .unwrap();

        let (title, desc, tag, meta) = sample_proposal_msgs("Delhi, India");
        contract
            .create_proposal(title, desc, tag, meta)
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
        let code_id = CodeId::store_code(&app);
        let proposer = "proposer".into_addr();
        let voter_a = "voter_a".into_addr();
        let voter_b = "voter_b".into_addr();
        let voter_c = "voter_c".into_addr();
        let contract = code_id
            .instantiate(Some(5000), Some(100))
            .call(&proposer)
            .unwrap();

        let (title, desc, tag, meta) = sample_proposal_msgs("Delhi, India");
        contract
            .create_proposal(title, desc, tag, meta)
            .call(&proposer)
            .unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_a).unwrap();

        contract
            .create_proposal(
                "Register voters".to_string(),
                "Bootstrap voter registry".to_string(),
                "bootstrap".to_string(),
                sample_metadata("Bengaluru, India"),
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
    fn successful_execute_runs_once() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let proposer = "proposer".into_addr();
        let voter_a = "voter_a".into_addr();
        let voter_b = "voter_b".into_addr();
        let contract = code_id
            .instantiate(Some(3000), Some(100))
            .call(&proposer)
            .unwrap();

        let (title, desc, tag, meta) = sample_proposal_msgs("Delhi, India");
        contract
            .create_proposal(title, desc, tag, meta)
            .call(&proposer)
            .unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_a).unwrap();
        contract.vote(1, VoteOption::Yes).call(&voter_b).unwrap();

        advance_time(&app, 200);

        contract.execute_proposal(1).call(&proposer).unwrap();

        let proposal = contract.get_proposal(1).unwrap();
        assert!(matches!(proposal.status, ProposalStatus::Executed));

        let again = contract.execute_proposal(1).call(&proposer);
        assert!(again.is_err());
        assert!(
            again
                .unwrap_err()
                .to_string()
                .contains(&ContractError::AlreadyFinalized.to_string())
        );
    }

    #[test]
    fn failed_proposal_cannot_execute_successfully() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let proposer = "proposer".into_addr();
        let voter_a = "voter_a".into_addr();
        let voter_b = "voter_b".into_addr();
        let contract = code_id
            .instantiate(Some(1000), Some(100))
            .call(&proposer)
            .unwrap();

        let (title, desc, tag, meta) = sample_proposal_msgs("Delhi, India");
        contract
            .create_proposal(title, desc, tag, meta)
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
        let code_id = CodeId::store_code(&app);
        let proposer = "proposer".into_addr();
        let contract = code_id
            .instantiate(Some(3000), Some(3600))
            .call(&proposer)
            .unwrap();

        let (title, desc, tag, meta) = sample_proposal_msgs("Delhi, India");
        contract
            .create_proposal(title, desc, tag, meta)
            .call(&proposer)
            .unwrap();
        contract
            .create_proposal(
                "Lake Pichola tree planting".to_string(),
                "Approve sapling budget for Udaipur shoreline.".to_string(),
                "tree_planting".to_string(),
                sample_metadata("Udaipur, India"),
            )
            .call(&proposer)
            .unwrap();

        let listed = contract.list_proposals(None, None).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[1].title, "Lake Pichola tree planting");
    }
}
