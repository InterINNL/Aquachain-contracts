#[cfg(test)]
mod tests {
    use crate::constants::DEFAULT_DENOM;
    use crate::contract::sv::mt::CodeId;
    use crate::contract::sv::mt::WaterCreditMarketplaceContractProxy;
    use crate::errors::ContractError;
    use sylvia::cw_multi_test::{App as MtApp, BankSudo, IntoAddr, SudoMsg};
    use sylvia::cw_std::{Timestamp, Uint128, coin};
    use sylvia::multitest::App;

    fn mint_coins(app: &App<MtApp>, to: impl Into<String>, amount: u128) {
        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: to.into(),
                amount: vec![coin(amount, DEFAULT_DENOM)],
            }))
            .unwrap();
    }

    fn advance_time(app: &App<MtApp>, seconds: u64) {
        app.app_mut().update_block(|block| {
            block.time = Timestamp::from_seconds(block.time.seconds() + seconds);
        });
    }

    #[test]
    fn mint_list_and_buy_atomic() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let seller = "seller".into_addr();
        let buyer = "buyer".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .mint_credits(seller.clone(), Uint128::new(100))
            .call(&admin)
            .unwrap();

        contract
            .list_credit(
                Uint128::new(40),
                Uint128::new(500),
                "Delhi NCR, India".to_string(),
                None,
            )
            .call(&seller)
            .unwrap();

        mint_coins(&app, buyer.clone(), 500);
        contract
            .buy_credit(1)
            .with_funds(&[coin(500, DEFAULT_DENOM)])
            .call(&buyer)
            .unwrap();

        let buyer_credits = contract.get_balance(buyer.clone()).unwrap();
        assert_eq!(buyer_credits, Uint128::new(40));

        let seller_credits = contract.get_balance(seller.clone()).unwrap();
        assert_eq!(seller_credits, Uint128::new(60));

        let seller_coins = app
            .app()
            .wrap()
            .query_balance(seller.clone(), DEFAULT_DENOM)
            .unwrap();
        assert_eq!(seller_coins.amount, Uint128::new(500));

        let listing = contract.get_listing(1).unwrap();
        assert!(!listing.active);
    }

    #[test]
    fn reject_buy_on_expired_listing() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let seller = "seller".into_addr();
        let buyer = "buyer".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .mint_credits(seller.clone(), Uint128::new(50))
            .call(&admin)
            .unwrap();

        let expires = app.app().block_info().time.seconds() + 60;
        contract
            .list_credit(
                Uint128::new(20),
                Uint128::new(200),
                "Bengaluru, India".to_string(),
                Some(expires),
            )
            .call(&seller)
            .unwrap();

        advance_time(&app, 120);
        mint_coins(&app, buyer.clone(), 200);

        let err = contract
            .buy_credit(1)
            .with_funds(&[coin(200, DEFAULT_DENOM)])
            .call(&buyer);
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains(&ContractError::ListingExpired.to_string())
        );
    }

    #[test]
    fn reject_buy_with_insufficient_funds() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let seller = "seller".into_addr();
        let buyer = "buyer".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .mint_credits(seller.clone(), Uint128::new(30))
            .call(&admin)
            .unwrap();

        contract
            .list_credit(
                Uint128::new(10),
                Uint128::new(300),
                "Udaipur, India".to_string(),
                None,
            )
            .call(&seller)
            .unwrap();

        mint_coins(&app, buyer.clone(), 100);
        let err = contract
            .buy_credit(1)
            .with_funds(&[coin(100, DEFAULT_DENOM)])
            .call(&buyer);
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains(&ContractError::WrongPrice.to_string())
        );
    }

    #[test]
    fn reject_list_with_insufficient_balance() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let seller = "seller".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .mint_credits(seller.clone(), Uint128::new(5))
            .call(&admin)
            .unwrap();

        let err = contract
            .list_credit(
                Uint128::new(10),
                Uint128::new(100),
                "Mumbai, India".to_string(),
                None,
            )
            .call(&seller);
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains(&ContractError::InsufficientCredits.to_string())
        );
    }

    #[test]
    fn cancel_listing_returns_credits() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let seller = "seller".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .mint_credits(seller.clone(), Uint128::new(25))
            .call(&admin)
            .unwrap();

        contract
            .list_credit(
                Uint128::new(15),
                Uint128::new(150),
                "Nashik, India".to_string(),
                None,
            )
            .call(&seller)
            .unwrap();

        assert_eq!(
            contract.get_balance(seller.clone()).unwrap(),
            Uint128::new(10)
        );

        contract.cancel_listing(1).call(&seller).unwrap();

        assert_eq!(
            contract.get_balance(seller.clone()).unwrap(),
            Uint128::new(25)
        );
        assert!(!contract.get_listing(1).unwrap().active);
    }

    #[test]
    fn transfer_credit_between_accounts() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let alice = "alice".into_addr();
        let bob = "bob".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .mint_credits(alice.clone(), Uint128::new(80))
            .call(&admin)
            .unwrap();

        contract
            .transfer_credit(bob.clone(), Uint128::new(30))
            .call(&alice)
            .unwrap();

        assert_eq!(contract.get_balance(alice).unwrap(), Uint128::new(50));
        assert_eq!(contract.get_balance(bob).unwrap(), Uint128::new(30));
    }
}
