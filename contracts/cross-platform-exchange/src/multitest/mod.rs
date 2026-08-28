#[cfg(test)]
mod tests {
    use crate::constants::DEFAULT_DENOM;
    use crate::contract::sv::mt::CodeId;
    use crate::contract::sv::mt::CrossPlatformExchangeContractProxy;
    use crate::contract::SwapDirection;
    use crate::errors::ContractError;
    use sylvia::cw_multi_test::{App as MtApp, BankSudo, IntoAddr, SudoMsg};
    use sylvia::cw_std::{Uint128, coin};
    use sylvia::multitest::App;

    fn mint_coins(app: &App<MtApp>, to: impl Into<String>, amount: u128) {
        app.app_mut()
            .sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: to.into(),
                amount: vec![coin(amount, DEFAULT_DENOM)],
            }))
            .unwrap();
    }

    #[test]
    fn swap_uses_registered_rate_exactly() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let trader = "trader".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .register_partner(
                "gujarat-water-unit".to_string(),
                "Gujarat water ledger".to_string(),
                "Gujarat, India".to_string(),
            )
            .call(&admin)
            .unwrap();
        contract
            .set_rate(
                "gujarat-water-unit".to_string(),
                Uint128::new(100),
                Uint128::new(10),
            )
            .call(&admin)
            .unwrap();

        mint_coins(&app, trader.clone(), 300);
        contract
            .swap(
                "gujarat-water-unit".to_string(),
                SwapDirection::BaseToPartner,
                Uint128::new(200),
            )
            .with_funds(&[coin(200, DEFAULT_DENOM)])
            .call(&trader)
            .unwrap();

        let locked = contract
            .get_locked_balance(trader.clone(), "gujarat-water-unit".to_string())
            .unwrap();
        assert_eq!(locked, Uint128::new(20));

        contract
            .swap(
                "gujarat-water-unit".to_string(),
                SwapDirection::PartnerToBase,
                Uint128::new(10),
            )
            .call(&trader)
            .unwrap();

        let locked_after = contract
            .get_locked_balance(trader.clone(), "gujarat-water-unit".to_string())
            .unwrap();
        assert_eq!(locked_after, Uint128::new(10));

        let trader_coins = app
            .app()
            .wrap()
            .query_balance(trader.clone(), DEFAULT_DENOM)
            .unwrap();
        assert_eq!(trader_coins.amount, Uint128::new(200));
    }

    #[test]
    fn admin_only_register_and_set_rate() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let stranger = "stranger".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        let reg_err = contract
            .register_partner(
                "yamuna-credit".to_string(),
                "Yamuna credit".to_string(),
                "Delhi, India".to_string(),
            )
            .call(&stranger);
        assert!(reg_err.is_err());
        assert!(
            reg_err
                .unwrap_err()
                .to_string()
                .contains(&ContractError::Unauthorized.to_string())
        );

        contract
            .register_partner(
                "yamuna-credit".to_string(),
                "Yamuna credit".to_string(),
                "Delhi, India".to_string(),
            )
            .call(&admin)
            .unwrap();

        let rate_err = contract
            .set_rate(
                "yamuna-credit".to_string(),
                Uint128::new(50),
                Uint128::new(5),
            )
            .call(&stranger);
        assert!(rate_err.is_err());
        assert!(
            rate_err
                .unwrap_err()
                .to_string()
                .contains(&ContractError::Unauthorized.to_string())
        );
    }

    #[test]
    fn unauthorized_withdraw_fails() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let owner = "owner".into_addr();
        let thief = "thief".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .register_partner(
                "bengaluru-aqua-point".to_string(),
                "Bengaluru aqua points".to_string(),
                "Bengaluru, India".to_string(),
            )
            .call(&admin)
            .unwrap();
        contract
            .set_rate(
                "bengaluru-aqua-point".to_string(),
                Uint128::new(100),
                Uint128::new(5),
            )
            .call(&admin)
            .unwrap();

        mint_coins(&app, owner.clone(), 100);
        contract
            .swap(
                "bengaluru-aqua-point".to_string(),
                SwapDirection::BaseToPartner,
                Uint128::new(100),
            )
            .with_funds(&[coin(100, DEFAULT_DENOM)])
            .call(&owner)
            .unwrap();

        let err = contract
            .withdraw("bengaluru-aqua-point".to_string(), Uint128::new(1))
            .call(&thief);
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains(&ContractError::InsufficientLocked.to_string())
        );

        contract
            .withdraw("bengaluru-aqua-point".to_string(), Uint128::new(5))
            .call(&owner)
            .unwrap();

        assert_eq!(
            contract
                .get_locked_balance(owner, "bengaluru-aqua-point".to_string())
                .unwrap(),
            Uint128::zero()
        );
    }

    #[test]
    fn reject_inexact_rate_amounts() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let trader = "trader".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .register_partner(
                "udaipur-lake-point".to_string(),
                "Udaipur lake points".to_string(),
                "Udaipur, Rajasthan, India".to_string(),
            )
            .call(&admin)
            .unwrap();
        contract
            .set_rate(
                "udaipur-lake-point".to_string(),
                Uint128::new(100),
                Uint128::new(10),
            )
            .call(&admin)
            .unwrap();

        mint_coins(&app, trader.clone(), 155);
        let err = contract
            .swap(
                "udaipur-lake-point".to_string(),
                SwapDirection::BaseToPartner,
                Uint128::new(155),
            )
            .with_funds(&[coin(155, DEFAULT_DENOM)])
            .call(&trader);
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains(&ContractError::InexactAmount.to_string())
        );
    }

    #[test]
    fn list_partners_and_get_rate() {
        let app = App::default();
        let code_id = CodeId::store_code(&app);
        let admin = "admin".into_addr();
        let contract = code_id
            .instantiate(Some(DEFAULT_DENOM.to_string()))
            .call(&admin)
            .unwrap();

        contract
            .register_partner(
                "nashik-aqua-unit".to_string(),
                "Nashik aqua units".to_string(),
                "Nashik, Maharashtra, India".to_string(),
            )
            .call(&admin)
            .unwrap();
        contract
            .set_rate(
                "nashik-aqua-unit".to_string(),
                Uint128::new(1000),
                Uint128::new(25),
            )
            .call(&admin)
            .unwrap();

        let partners = contract.list_partners(None, None, None).unwrap();
        assert_eq!(partners.len(), 1);
        assert_eq!(partners[0].region, "Nashik, Maharashtra, India");

        let rate = contract
            .get_rate("nashik-aqua-unit".to_string())
            .unwrap();
        assert_eq!(rate.base_amount, Uint128::new(1000));
        assert_eq!(rate.partner_amount, Uint128::new(25));
    }
}
