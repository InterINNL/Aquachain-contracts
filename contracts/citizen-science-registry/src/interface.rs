// use crate::contract::Project;
// use sylvia::ctx::{ExecCtx, QueryCtx};
// use sylvia::cw_std::{CustomMsg, CustomQuery, Response, StdError, StdResult, Uint128};
// use sylvia::interface;

// #[interface]
// pub trait WaterWellDonationInterface {
//     type Error: From<StdError>;
//     type ExecC: CustomMsg + CustomQuery;
//     type QueryC: CustomQuery + CustomMsg;

//     #[sv::msg(exec)]
//     fn create_project(
//         &self,
//         ctx: ExecCtx<Self::QueryC>,
//         goal: Uint128,
//     ) -> StdResult<Response<Self::ExecC>>;

//     #[sv::msg(exec)]
//     fn donate(
//         &self,
//         ctx: ExecCtx<Self::QueryC>,
//         project_id: u64,
//     ) -> StdResult<Response<Self::ExecC>>;

//     #[sv::msg(exec)]
//     fn disburse(
//         &self,
//         ctx: ExecCtx<Self::QueryC>,
//         project_id: u64,
//     ) -> StdResult<Response<Self::ExecC>>;

//     #[sv::msg(query)]
//     fn get_project(&self, ctx: QueryCtx<Self::QueryC>, project_id: u64) -> StdResult<Project>;

//     #[sv::msg(query)]
//     fn list_projects(&self, ctx: QueryCtx<Self::QueryC>) -> StdResult<Vec<Project>>;
// }
