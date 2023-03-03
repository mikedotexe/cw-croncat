use cosmwasm_std::{coins, to_binary, Addr, BlockInfo, Coin, Uint128, WasmMsg};
use croncat_sdk_factory::msg::{ContractMetadataResponse, ModuleInstantiateInfo, VersionKind};
use croncat_sdk_manager::types::{Config, UpdateConfig};

use cw20::{Cw20Coin, Cw20CoinVerified};
use cw_multi_test::{App, AppBuilder, Executor};

use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

use super::{
    contracts, ADMIN, AGENT0, AGENT1, AGENT2, AGENT3, AGENT4, AGENT_BENEFICIARY, ANYONE, DENOM,
    PARTICIPANT0, PARTICIPANT1, PARTICIPANT2, PARTICIPANT3, PARTICIPANT4, PARTICIPANT5,
    PARTICIPANT6, PAUSE_ADMIN, VERSION, VERY_RICH,
};

pub(crate) fn default_app() -> App {
    AppBuilder::new().build(|router, _, storage| {
        let accounts: Vec<(u128, String)> = vec![
            (6_000_000, ADMIN.to_string()),
            (600_000, PAUSE_ADMIN.to_string()),
            (500_000, ANYONE.to_string()),
            (2_000_000, AGENT0.to_string()),
            (2_000_000, AGENT1.to_string()),
            (2_000_000, AGENT2.to_string()),
            (2_000_000, AGENT3.to_string()),
            (2_000_000, AGENT4.to_string()),
            (5_000_000, PARTICIPANT0.to_string()),
            (5_000_000, PARTICIPANT1.to_string()),
            (5_000_000, PARTICIPANT2.to_string()),
            (5_000_000, PARTICIPANT3.to_string()),
            (5_000_000, PARTICIPANT4.to_string()),
            (5_000_000, PARTICIPANT5.to_string()),
            (5_000_000, PARTICIPANT6.to_string()),
            (2_000_000, AGENT_BENEFICIARY.to_string()),
            (u128::MAX.saturating_sub(1000), VERY_RICH.to_string()),
        ];
        for (amt, address) in accounts {
            router
                .bank
                .init_balance(storage, &Addr::unchecked(address), coins(amt, DENOM))
                .unwrap();
        }
    })
}

pub(crate) fn init_factory(app: &mut App) -> Addr {
    let code_id = app.store_code(contracts::croncat_factory_contract());
    app.instantiate_contract(
        code_id,
        Addr::unchecked(ADMIN),
        &croncat_factory::msg::InstantiateMsg { owner_addr: None },
        &[],
        "croncat_factory",
        None,
    )
    .unwrap()
}

pub(crate) fn init_manager(
    app: &mut App,
    msg: &InstantiateMsg,
    factory_addr: &Addr,
    funds: &[Coin],
) -> Addr {
    let code_id = app.store_code(contracts::croncat_manager_contract());

    let module_instantiate_info = ModuleInstantiateInfo {
        code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(msg).unwrap(),
        contract_name: "manager".to_owned(),
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        factory_addr.to_owned(),
        &croncat_factory::msg::ExecuteMsg::Deploy {
            kind: VersionKind::Manager,
            module_instantiate_info,
        },
        funds,
    )
    .unwrap();

    let metadata: ContractMetadataResponse = app
        .wrap()
        .query_wasm_smart(
            factory_addr,
            &croncat_factory::msg::QueryMsg::LatestContract {
                contract_name: "manager".to_owned(),
            },
        )
        .unwrap();
    metadata.metadata.unwrap().contract_addr
}

pub(crate) fn init_boolean(app: &mut App) -> Addr {
    let code_id = app.store_code(contracts::cw_boolean_contract());
    let inst_msg = cw_boolean_contract::msgs::instantiate_msg::InstantiateMsg {};
    app.instantiate_contract(
        code_id,
        Addr::unchecked(ADMIN),
        &inst_msg,
        &[],
        "cw-boolean-contract",
        None,
    )
    .unwrap()
}

pub(crate) fn init_tasks(app: &mut App, factory_addr: &Addr) -> Addr {
    let code_id = app.store_code(contracts::croncat_tasks_contract());
    let msg = croncat_tasks::msg::InstantiateMsg {
        version: Some(VERSION.to_owned()),
        chain_name: "atom".to_owned(),
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        croncat_manager_key: ("manager".to_owned(), [0, 1]),
        croncat_agents_key: ("agents".to_owned(), [0, 1]),
        slot_granularity_time: None,
        gas_base_fee: None,
        gas_action_fee: None,
        gas_query_fee: None,
        gas_limit: None,
    };
    let module_instantiate_info = ModuleInstantiateInfo {
        code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&msg).unwrap(),
        contract_name: "tasks".to_owned(),
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        factory_addr.to_owned(),
        &croncat_factory::msg::ExecuteMsg::Deploy {
            kind: VersionKind::Tasks,
            module_instantiate_info,
        },
        &[],
    )
    .unwrap();

    let metadata: ContractMetadataResponse = app
        .wrap()
        .query_wasm_smart(
            factory_addr,
            &croncat_factory::msg::QueryMsg::LatestContract {
                contract_name: "tasks".to_owned(),
            },
        )
        .unwrap();
    metadata.metadata.unwrap().contract_addr
}

pub(crate) fn init_agents(app: &mut App, factory_addr: &Addr) -> Addr {
    let code_id = app.store_code(contracts::croncat_agents_contract());
    let msg = croncat_agents::msg::InstantiateMsg {
        version: Some(VERSION.to_owned()),
        croncat_manager_key: ("manager".to_owned(), [0, 1]),
        croncat_tasks_key: ("tasks".to_owned(), [0, 1]),
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        agent_nomination_duration: None,
        min_tasks_per_agent: None,
        min_coins_for_agent_registration: None,
        agents_eject_threshold: None,
        min_active_agent_count: None,
    };
    let module_instantiate_info = ModuleInstantiateInfo {
        code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&msg).unwrap(),
        contract_name: "agents".to_owned(),
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        factory_addr.to_owned(),
        &croncat_factory::msg::ExecuteMsg::Deploy {
            kind: VersionKind::Agents,
            module_instantiate_info,
        },
        &[],
    )
    .unwrap();

    let metadata: ContractMetadataResponse = app
        .wrap()
        .query_wasm_smart(
            factory_addr,
            &croncat_factory::msg::QueryMsg::LatestContract {
                contract_name: "agents".to_owned(),
            },
        )
        .unwrap();
    metadata.metadata.unwrap().contract_addr
}

pub(crate) fn init_mod_balances(app: &mut App, factory_addr: &Addr) -> Addr {
    let code_id = app.store_code(contracts::mod_balances_contract());
    let msg = croncat_mod_balances::msg::InstantiateMsg {
        version: Some(VERSION.to_owned()),
    };
    let module_instantiate_info = ModuleInstantiateInfo {
        code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&msg).unwrap(),
        contract_name: "mod-balances".to_owned(),
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        factory_addr.to_owned(),
        &croncat_factory::msg::ExecuteMsg::Deploy {
            kind: VersionKind::Library,
            module_instantiate_info,
        },
        &[],
    )
    .unwrap();

    let metadata: ContractMetadataResponse = app
        .wrap()
        .query_wasm_smart(
            factory_addr,
            &croncat_factory::msg::QueryMsg::LatestContract {
                contract_name: "mod-balances".to_owned(),
            },
        )
        .unwrap();
    metadata.metadata.unwrap().contract_addr
}

#[allow(unused)]
pub(crate) fn init_mod_generic(app: &mut App, factory_addr: &Addr) -> Addr {
    let code_id = app.store_code(contracts::mod_generic_contract());
    let msg = croncat_mod_generic::msg::InstantiateMsg {
        version: Some(VERSION.to_owned()),
    };
    let module_instantiate_info = ModuleInstantiateInfo {
        code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&msg).unwrap(),
        contract_name: "mod-generic".to_owned(),
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        factory_addr.to_owned(),
        &croncat_factory::msg::ExecuteMsg::Deploy {
            kind: VersionKind::Library,
            module_instantiate_info,
        },
        &[],
    )
    .unwrap();

    let metadata: ContractMetadataResponse = app
        .wrap()
        .query_wasm_smart(
            factory_addr,
            &croncat_factory::msg::QueryMsg::LatestContract {
                contract_name: "mod-generic".to_owned(),
            },
        )
        .unwrap();
    metadata.metadata.unwrap().contract_addr
}

// Note: gonna work only with first agent, other have to get nominated
pub(crate) fn activate_agent(app: &mut App, agents_contract: &Addr) {
    app.execute_contract(
        Addr::unchecked(AGENT0),
        agents_contract.clone(),
        &croncat_agents::msg::ExecuteMsg::RegisterAgent {
            payable_account_id: None,
        },
        &[],
    )
    .unwrap();
}

pub(crate) fn init_cw20(app: &mut App) -> Addr {
    let code_id = app.store_code(contracts::cw20_contract());
    app.instantiate_contract(
        code_id,
        Addr::unchecked(ADMIN),
        &cw20_base::msg::InstantiateMsg {
            name: "coin_name".to_owned(),
            symbol: "con".to_owned(),
            decimals: 6,
            initial_balances: vec![
                Cw20Coin {
                    address: ADMIN.to_owned(),
                    amount: Uint128::new(100_000_000),
                },
                Cw20Coin {
                    address: PARTICIPANT0.to_owned(),
                    amount: Uint128::new(100_000_000),
                },
            ],
            mint: None,
            marketing: None,
        },
        &[],
        "cw20",
        None,
    )
    .unwrap()
}

pub(crate) fn default_instantiate_message() -> InstantiateMsg {
    InstantiateMsg {
        version: Some(VERSION.to_owned()),
        croncat_tasks_key: ("tasks".to_owned(), [0, 1]),
        croncat_agents_key: ("agents".to_owned(), [0, 1]),
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        gas_price: None,
        treasury_addr: None,
        cw20_whitelist: None,
    }
}

pub(crate) fn query_manager_config(app: &App, manager: &Addr) -> Config {
    app.wrap()
        .query_wasm_smart(manager, &QueryMsg::Config {})
        .unwrap()
}

pub(crate) fn query_manager_balances(app: &App, manager: &Addr) -> Uint128 {
    app.wrap()
        .query_wasm_smart(manager, &QueryMsg::TreasuryBalance {})
        .unwrap()
}

pub(crate) fn query_users_manager(
    app: &App,
    manager: &Addr,
    wallet: impl Into<String>,
) -> Vec<Cw20CoinVerified> {
    app.wrap()
        .query_wasm_smart(
            manager,
            &QueryMsg::UsersBalances {
                address: wallet.into(),
                from_index: None,
                limit: None,
            },
        )
        .unwrap()
}

pub(crate) fn add_little_time(block: &mut BlockInfo) {
    block.time = block.time.plus_seconds(19);
    block.height += 1;
}

pub(crate) fn support_new_cw20(
    app: &mut App,
    factory_addr: Addr,
    manager_addr: &Addr,
    new_cw20_addr: &str,
) {
    app.execute_contract(
        Addr::unchecked(ADMIN),
        factory_addr,
        &croncat_sdk_factory::msg::FactoryExecuteMsg::Proxy {
            msg: WasmMsg::Execute {
                contract_addr: manager_addr.to_string(),
                msg: to_binary(&ExecuteMsg::UpdateConfig(Box::new(UpdateConfig {
                    agent_fee: None,
                    treasury_fee: None,
                    gas_price: None,
                    croncat_tasks_key: None,
                    croncat_agents_key: None,
                    treasury_addr: None,
                    cw20_whitelist: Some(vec![new_cw20_addr.to_owned()]),
                })))
                .unwrap(),
                funds: vec![],
            },
        },
        &[],
    )
    .unwrap();
}

// Useful for debugging in case task got suddenly stuck
#[allow(unused)]
pub(crate) fn check_task_chain(app: &App, tasks_contract: &Addr, agents_contract: &Addr) {
    let current_task: Option<croncat_sdk_tasks::types::TaskResponse> = app
        .wrap()
        .query_wasm_smart(
            tasks_contract.clone(),
            &croncat_tasks::msg::QueryMsg::CurrentTask {},
        )
        .unwrap();
    let total_tasks: croncat_sdk_tasks::types::SlotTasksTotalResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_contract,
            &croncat_sdk_tasks::msg::TasksQueryMsg::SlotTasksTotal { offset: None },
        )
        .unwrap();
    let agents: croncat_sdk_agents::msg::GetAgentIdsResponse = app
        .wrap()
        .query_wasm_smart(
            agents_contract.clone(),
            &croncat_sdk_agents::msg::QueryMsg::GetAgentIds {
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    let tasks_for_agent: Option<croncat_sdk_agents::msg::AgentTaskResponse> = app
        .wrap()
        .query_wasm_smart(
            agents_contract,
            &croncat_sdk_agents::msg::QueryMsg::GetAgentTasks {
                account_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
}
