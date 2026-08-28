use std::str::FromStr;

use crate::constants::{
    CITIZEN_SCIENCE_REGISTRY, COMMUNITY_BOUNTY, DEFAULT_DENOM, WATER_CREDIT_MARKETPLACE,
};
use crate::contract::Proposal;
use crate::errors::ContractError;
use cosmwasm_schema::cw_serde;
use serde_json::{Value, json};
use sylvia::cw_std::{Addr, Coin, Storage, Uint128, WasmMsg, to_json_binary};

pub const ACTION_POST_BOUNTY: &str = "post_bounty";
pub const ACTION_MINT_CREDITS: &str = "mint_credits";
pub const ACTION_REWARD_SENSOR: &str = "reward_sensor";

pub const ALLOWED_ACTIONS: [&str; 3] =
    [ACTION_POST_BOUNTY, ACTION_MINT_CREDITS, ACTION_REWARD_SENSOR];

#[cw_serde]
struct PostBountyMetadata {
    location: String,
    deadline: u64,
    #[serde(default)]
    reward: Option<String>,
}

#[cw_serde]
struct MintCreditsMetadata {
    recipient: String,
    amount: String,
}

#[cw_serde]
struct RewardSensorMetadata {
    entry_id: u64,
    #[serde(default)]
    reward: Option<String>,
}

pub fn is_allowed_action(action_tag: &str) -> bool {
    ALLOWED_ACTIONS.contains(&action_tag)
}

pub fn action_target_configured(storage: &dyn Storage, action_tag: &str) -> Result<(), ContractError> {
    let configured = match action_tag {
        ACTION_POST_BOUNTY => COMMUNITY_BOUNTY.may_load(storage)?.is_some(),
        ACTION_MINT_CREDITS => WATER_CREDIT_MARKETPLACE.may_load(storage)?.is_some(),
        ACTION_REWARD_SENSOR => CITIZEN_SCIENCE_REGISTRY.may_load(storage)?.is_some(),
        _ => false,
    };
    if configured {
        Ok(())
    } else {
        Err(ContractError::ActionTargetNotConfigured)
    }
}

pub fn validate_action_metadata(action_tag: &str, metadata: &Value) -> Result<(), ContractError> {
    match action_tag {
        ACTION_POST_BOUNTY => {
            let parsed: PostBountyMetadata = serde_json::from_value(metadata.clone())
                .map_err(|_| ContractError::InvalidActionMetadata)?;
            if parsed.location.trim().is_empty() || parsed.deadline == 0 {
                return Err(ContractError::InvalidActionMetadata);
            }
        }
        ACTION_MINT_CREDITS => {
            let parsed: MintCreditsMetadata = serde_json::from_value(metadata.clone())
                .map_err(|_| ContractError::InvalidActionMetadata)?;
            if parsed.recipient.trim().is_empty() || parsed.amount.trim().is_empty() {
                return Err(ContractError::InvalidActionMetadata);
            }
            if Uint128::from_str(parsed.amount.trim())
                .map_err(|_| ContractError::InvalidActionMetadata)?
                .is_zero()
            {
                return Err(ContractError::InvalidActionMetadata);
            }
        }
        ACTION_REWARD_SENSOR => {
            let parsed: RewardSensorMetadata = serde_json::from_value(metadata.clone())
                .map_err(|_| ContractError::InvalidActionMetadata)?;
            if parsed.entry_id == 0 {
                return Err(ContractError::InvalidActionMetadata);
            }
        }
        _ => return Err(ContractError::UnsupportedAction),
    }
    Ok(())
}

pub fn build_execute_message(
    storage: &dyn Storage,
    proposal: &Proposal,
    attached_funds: &[Coin],
) -> Result<WasmMsg, ContractError> {
    let metadata: Value = serde_json::from_str(&proposal.metadata_str)
        .map_err(|_| ContractError::InvalidActionMetadata)?;
    let denom = DEFAULT_DENOM.load(storage)?;

    match proposal.action_tag.as_str() {
        ACTION_POST_BOUNTY => build_post_bounty(storage, proposal, &metadata, attached_funds, &denom),
        ACTION_MINT_CREDITS => build_mint_credits(storage, &metadata),
        ACTION_REWARD_SENSOR => {
            build_reward_sensor(storage, &metadata, attached_funds, &denom)
        }
        _ => Err(ContractError::UnsupportedAction),
    }
}

fn build_post_bounty(
    storage: &dyn Storage,
    proposal: &Proposal,
    metadata: &Value,
    attached_funds: &[Coin],
    denom: &str,
) -> Result<WasmMsg, ContractError> {
    let target = COMMUNITY_BOUNTY
        .load(storage)
        .map_err(|_| ContractError::ActionTargetNotConfigured)?;
    let parsed: PostBountyMetadata = serde_json::from_value(metadata.clone())
        .map_err(|_| ContractError::InvalidActionMetadata)?;

    let funds = resolve_reward_funds(attached_funds, denom, parsed.reward.as_deref())?;

    let msg = json!({
        "post_bounty": {
            "title": proposal.title,
            "description": proposal.description,
            "location": parsed.location.trim(),
            "deadline": parsed.deadline,
        }
    });

    Ok(WasmMsg::Execute {
        contract_addr: target.to_string(),
        msg: to_json_binary(&msg)?,
        funds,
    })
}

fn build_mint_credits(storage: &dyn Storage, metadata: &Value) -> Result<WasmMsg, ContractError> {
    let target = WATER_CREDIT_MARKETPLACE
        .load(storage)
        .map_err(|_| ContractError::ActionTargetNotConfigured)?;
    let parsed: MintCreditsMetadata = serde_json::from_value(metadata.clone())
        .map_err(|_| ContractError::InvalidActionMetadata)?;
    let recipient = Addr::unchecked(parsed.recipient.trim());
    let amount = Uint128::from_str(parsed.amount.trim())
        .map_err(|_| ContractError::InvalidActionMetadata)?;

    let msg = json!({
        "mint_credits": {
            "recipient": recipient,
            "amount": amount,
        }
    });

    Ok(WasmMsg::Execute {
        contract_addr: target.to_string(),
        msg: to_json_binary(&msg)?,
        funds: vec![],
    })
}

fn build_reward_sensor(
    storage: &dyn Storage,
    metadata: &Value,
    attached_funds: &[Coin],
    denom: &str,
) -> Result<WasmMsg, ContractError> {
    let target = CITIZEN_SCIENCE_REGISTRY
        .load(storage)
        .map_err(|_| ContractError::ActionTargetNotConfigured)?;
    let parsed: RewardSensorMetadata = serde_json::from_value(metadata.clone())
        .map_err(|_| ContractError::InvalidActionMetadata)?;

    let funds = resolve_reward_funds(attached_funds, denom, parsed.reward.as_deref())?;

    let msg = json!({
        "reward_submitter": {
            "entry_id": parsed.entry_id,
        }
    });

    Ok(WasmMsg::Execute {
        contract_addr: target.to_string(),
        msg: to_json_binary(&msg)?,
        funds,
    })
}

fn resolve_reward_funds(
    attached_funds: &[Coin],
    denom: &str,
    metadata_reward: Option<&str>,
) -> Result<Vec<Coin>, ContractError> {
    if let Some(reward_str) = metadata_reward {
        let expected = Uint128::from_str(reward_str.trim())
            .map_err(|_| ContractError::InvalidActionMetadata)?;
        if expected.is_zero() {
            return Err(ContractError::InvalidActionMetadata);
        }
        let attached = attached_funds
            .iter()
            .find(|coin| coin.denom == denom)
            .ok_or(ContractError::MissingFunds)?;
        if attached.amount != expected {
            return Err(ContractError::InvalidFundsAmount);
        }
        return Ok(vec![attached.clone()]);
    }

    let attached = attached_funds
        .iter()
        .find(|coin| coin.denom == denom)
        .ok_or(ContractError::MissingFunds)?;
    if attached.amount.is_zero() {
        return Err(ContractError::MissingFunds);
    }
    Ok(vec![attached.clone()])
}
