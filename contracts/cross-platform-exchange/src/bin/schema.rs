use cross_platform_exchange::contract::sv::{
    ContractExecMsg, ContractQueryMsg, InstantiateMsg,
};
use cosmwasm_schema::write_api;

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ContractExecMsg,
        query: ContractQueryMsg,
    }
}
