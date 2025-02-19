use cosmwasm_std::Empty;
use cw_multi_test::{Contract, ContractWrapper};

pub fn croncat_factory_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        croncat_factory::contract::execute,
        croncat_factory::contract::instantiate,
        croncat_factory::contract::query,
    )
    .with_reply(croncat_factory::contract::reply);
    Box::new(contract)
}

pub fn croncat_manager_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        croncat_manager::contract::execute,
        croncat_manager::contract::instantiate,
        croncat_manager::contract::query,
    )
    .with_reply(croncat_manager::contract::reply);
    Box::new(contract)
}

pub fn croncat_tasks_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        croncat_tasks::contract::execute,
        croncat_tasks::contract::instantiate,
        croncat_tasks::contract::query,
    );
    Box::new(contract)
}

pub fn croncat_agents_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        croncat_agents::contract::execute,
        croncat_agents::contract::instantiate,
        croncat_agents::contract::query,
    );
    Box::new(contract)
}
