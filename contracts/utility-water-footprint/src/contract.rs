use crate::constants::{
    ADMIN, CERT_BY_COMPANY_PERIOD, CERTIFICATES, COMPANIES, DEFAULT_DENOM, DENOM,
    MIN_SAVINGS_RATIO_BPS, NEXT_CERT_ID, NEXT_COMPANY_ID, NEXT_LOG_ID, USAGE_LOGS, VERIFIERS,
};
use crate::errors::ContractError;
use cosmwasm_schema::cw_serde;
use cw_storage_plus::Bound;
use serde_json::{Value, to_string};
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_std::{Addr, Order, Response, StdResult, Uint128};
use sylvia::entry_points;

#[cw_serde]
pub struct Company {
    pub id: u64,
    pub owner: Addr,
    pub name: String,
    pub metadata_str: String,
}

#[cw_serde]
pub struct UsageLog {
    pub id: u64,
    pub company_id: u64,
    pub period: String,
    pub usage: Uint128,
    pub savings: Uint128,
    pub validated: bool,
    pub validator: Option<Addr>,
    pub created_at: u64,
}

#[cw_serde]
pub struct Certificate {
    pub id: u64,
    pub company_id: u64,
    pub period: String,
    pub total_usage: Uint128,
    pub total_savings: Uint128,
    pub issuer: Addr,
    pub issued_at: u64,
}

pub struct UtilityWaterFootprintContract;

#[cfg_attr(not(feature = "library"), entry_points)]
#[contract]
#[sv::error(ContractError)]
impl UtilityWaterFootprintContract {
    pub const fn new() -> Self {
        Self
    }

    #[sv::msg(instantiate)]
    fn instantiate(&self, ctx: InstantiateCtx, denom: Option<String>) -> StdResult<Response> {
        ADMIN.save(ctx.deps.storage, &ctx.info.sender)?;
        NEXT_COMPANY_ID.save(ctx.deps.storage, &1)?;
        NEXT_LOG_ID.save(ctx.deps.storage, &1)?;
        NEXT_CERT_ID.save(ctx.deps.storage, &1)?;

        let denom_to_store = denom.unwrap_or_else(|| DEFAULT_DENOM.to_string());
        DENOM.save(ctx.deps.storage, &denom_to_store)?;

        Ok(Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("denom", denom_to_store))
    }

    #[sv::msg(exec)]
    fn register_company(&self, ctx: ExecCtx, name: String, metadata: Value) -> StdResult<Response> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(ContractError::EmptyName.into());
        }

        let metadata_str = serialize_metadata(&metadata)?;

        let id = NEXT_COMPANY_ID.load(ctx.deps.storage)?;
        let company = Company {
            id,
            owner: ctx.info.sender.clone(),
            name: name.clone(),
            metadata_str,
        };

        COMPANIES.save(ctx.deps.storage, id, &company)?;
        NEXT_COMPANY_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "register_company")
            .add_attribute("company_id", id.to_string())
            .add_attribute("owner", ctx.info.sender.to_string())
            .add_attribute("name", name))
    }

    #[sv::msg(exec)]
    fn log_usage(
        &self,
        ctx: ExecCtx,
        company_id: u64,
        period: String,
        usage: Uint128,
        savings: Uint128,
    ) -> StdResult<Response> {
        let company = COMPANIES.load(ctx.deps.storage, company_id)?;
        if ctx.info.sender != company.owner {
            return Err(ContractError::Unauthorized.into());
        }

        let period = period.trim().to_string();
        if period.is_empty() {
            return Err(ContractError::EmptyPeriod.into());
        }
        if usage.is_zero() {
            return Err(ContractError::ZeroUsage.into());
        }
        if savings > usage {
            return Err(ContractError::IllogicalMetrics.into());
        }

        let id = NEXT_LOG_ID.load(ctx.deps.storage)?;
        let log = UsageLog {
            id,
            company_id,
            period: period.clone(),
            usage,
            savings,
            validated: false,
            validator: None,
            created_at: ctx.env.block.time.seconds(),
        };

        USAGE_LOGS.save(ctx.deps.storage, id, &log)?;
        NEXT_LOG_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "log_usage")
            .add_attribute("log_id", id.to_string())
            .add_attribute("company_id", company_id.to_string())
            .add_attribute("period", period))
    }

    #[sv::msg(exec)]
    fn validate_log(&self, ctx: ExecCtx, log_id: u64) -> StdResult<Response> {
        ensure_admin_or_verifier(&ctx)?;

        let mut log = USAGE_LOGS.load(ctx.deps.storage, log_id)?;
        if log.validated {
            return Err(ContractError::AlreadyValidated.into());
        }

        log.validated = true;
        log.validator = Some(ctx.info.sender.clone());
        USAGE_LOGS.save(ctx.deps.storage, log_id, &log)?;

        Ok(Response::new()
            .add_attribute("action", "validate_log")
            .add_attribute("log_id", log_id.to_string())
            .add_attribute("validator", ctx.info.sender.to_string()))
    }

    #[sv::msg(exec)]
    fn add_verifier(&self, ctx: ExecCtx, verifier: Addr) -> StdResult<Response> {
        ensure_admin(&ctx)?;

        if VERIFIERS
            .may_load(ctx.deps.storage, verifier.clone())?
            .unwrap_or(false)
        {
            return Err(ContractError::VerifierAlreadyExists.into());
        }

        VERIFIERS.save(ctx.deps.storage, verifier.clone(), &true)?;
        Ok(Response::new()
            .add_attribute("action", "add_verifier")
            .add_attribute("verifier", verifier.to_string()))
    }

    #[sv::msg(exec)]
    fn remove_verifier(&self, ctx: ExecCtx, verifier: Addr) -> StdResult<Response> {
        ensure_admin(&ctx)?;

        if !VERIFIERS
            .may_load(ctx.deps.storage, verifier.clone())?
            .unwrap_or(false)
        {
            return Err(ContractError::VerifierNotFound.into());
        }

        VERIFIERS.remove(ctx.deps.storage, verifier.clone());
        Ok(Response::new()
            .add_attribute("action", "remove_verifier")
            .add_attribute("verifier", verifier.to_string()))
    }

    #[sv::msg(exec)]
    fn issue_certificate(
        &self,
        ctx: ExecCtx,
        company_id: u64,
        period: String,
    ) -> StdResult<Response> {
        ensure_admin_or_verifier(&ctx)?;

        let period = period.trim().to_string();
        if period.is_empty() {
            return Err(ContractError::EmptyPeriod.into());
        }

        COMPANIES.load(ctx.deps.storage, company_id)?;

        if CERT_BY_COMPANY_PERIOD
            .may_load(ctx.deps.storage, (company_id, period.clone()))?
            .is_some()
        {
            return Err(ContractError::AlreadyIssued.into());
        }

        let (total_usage, total_savings) =
            sum_validated_logs(ctx.deps.storage, company_id, &period)?;
        if !meets_certificate_threshold(total_usage, total_savings) {
            return Err(ContractError::CriteriaNotMet.into());
        }

        let id = NEXT_CERT_ID.load(ctx.deps.storage)?;
        let certificate = Certificate {
            id,
            company_id,
            period: period.clone(),
            total_usage,
            total_savings,
            issuer: ctx.info.sender.clone(),
            issued_at: ctx.env.block.time.seconds(),
        };

        CERTIFICATES.save(ctx.deps.storage, id, &certificate)?;
        CERT_BY_COMPANY_PERIOD.save(ctx.deps.storage, (company_id, period.clone()), &id)?;
        NEXT_CERT_ID.save(ctx.deps.storage, &(id + 1))?;

        Ok(Response::new()
            .add_attribute("action", "issue_certificate")
            .add_attribute("certificate_id", id.to_string())
            .add_attribute("company_id", company_id.to_string())
            .add_attribute("period", period))
    }

    #[sv::msg(query)]
    fn get_company(&self, ctx: QueryCtx, company_id: u64) -> StdResult<Company> {
        COMPANIES.load(ctx.deps.storage, company_id)
    }

    #[sv::msg(query)]
    fn list_companies(
        &self,
        ctx: QueryCtx,
        limit: Option<u32>,
        start_after: Option<u64>,
    ) -> StdResult<Vec<Company>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);

        COMPANIES
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| item.map(|(_, c)| c))
            .collect()
    }

    #[sv::msg(query)]
    fn list_logs(
        &self,
        ctx: QueryCtx,
        company_id: Option<u64>,
        limit: Option<u32>,
        start_after: Option<u64>,
    ) -> StdResult<Vec<UsageLog>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);

        USAGE_LOGS
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .filter_map(|item| {
                let (_, log) = item.ok()?;
                if let Some(id) = company_id
                    && log.company_id != id
                {
                    return None;
                }
                Some(Ok(log))
            })
            .take(limit)
            .collect()
    }

    #[sv::msg(query)]
    fn get_certificate(&self, ctx: QueryCtx, certificate_id: u64) -> StdResult<Certificate> {
        CERTIFICATES.load(ctx.deps.storage, certificate_id)
    }

    #[sv::msg(query)]
    fn list_certificates(
        &self,
        ctx: QueryCtx,
        company_id: Option<u64>,
        limit: Option<u32>,
        start_after: Option<u64>,
    ) -> StdResult<Vec<Certificate>> {
        let limit = limit.unwrap_or(10).min(30) as usize;
        let start = start_after.map(Bound::exclusive);

        CERTIFICATES
            .range(ctx.deps.storage, start, None, Order::Ascending)
            .filter_map(|item| {
                let (_, cert) = item.ok()?;
                if let Some(id) = company_id
                    && cert.company_id != id
                {
                    return None;
                }
                Some(Ok(cert))
            })
            .take(limit)
            .collect()
    }

    #[sv::msg(query)]
    fn is_verifier(&self, ctx: QueryCtx, address: Addr) -> StdResult<bool> {
        Ok(VERIFIERS
            .may_load(ctx.deps.storage, address)?
            .unwrap_or(false))
    }
}

fn serialize_metadata(metadata: &Value) -> StdResult<String> {
    match metadata {
        Value::String(s) if s.is_empty() => Err(ContractError::InvalidJson.into()),
        Value::Object(map) if map.is_empty() => Err(ContractError::InvalidJson.into()),
        Value::Array(arr) if arr.is_empty() => Err(ContractError::InvalidJson.into()),
        _ => to_string(metadata).map_err(|_| ContractError::InvalidJson.into()),
    }
}

fn ensure_admin(ctx: &ExecCtx) -> StdResult<()> {
    let admin = ADMIN.load(ctx.deps.storage)?;
    if ctx.info.sender != admin {
        return Err(ContractError::Unauthorized.into());
    }
    Ok(())
}

fn ensure_admin_or_verifier(ctx: &ExecCtx) -> StdResult<()> {
    let admin = ADMIN.load(ctx.deps.storage)?;
    if ctx.info.sender == admin {
        return Ok(());
    }
    if VERIFIERS
        .may_load(ctx.deps.storage, ctx.info.sender.clone())?
        .unwrap_or(false)
    {
        return Ok(());
    }
    Err(ContractError::Unauthorized.into())
}

fn sum_validated_logs(
    storage: &dyn sylvia::cw_std::Storage,
    company_id: u64,
    period: &str,
) -> StdResult<(Uint128, Uint128)> {
    let mut total_usage = Uint128::zero();
    let mut total_savings = Uint128::zero();

    for item in USAGE_LOGS.range(storage, None, None, Order::Ascending) {
        let (_, log) = item?;
        if log.company_id == company_id && log.period == period && log.validated {
            total_usage += log.usage;
            total_savings += log.savings;
        }
    }

    Ok((total_usage, total_savings))
}

fn meets_certificate_threshold(total_usage: Uint128, total_savings: Uint128) -> bool {
    if total_usage.is_zero() || total_savings.is_zero() {
        return false;
    }
    // savings / usage >= MIN_SAVINGS_RATIO_BPS / 10000
    total_savings.u128().saturating_mul(10_000) / total_usage.u128() >= MIN_SAVINGS_RATIO_BPS
}
