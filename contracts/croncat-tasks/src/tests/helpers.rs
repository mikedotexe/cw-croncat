use super::{
    contracts, ADMIN, AGENT0, AGENT1, AGENT2, AGENT3, AGENT4, AGENT_BENEFICIARY, ANYONE, DENOM,
    PARTICIPANT0, PARTICIPANT1, PARTICIPANT2, PARTICIPANT3, PARTICIPANT4, PARTICIPANT5,
    PARTICIPANT6, PAUSE_ADMIN, VERSION, VERY_RICH,
};
use crate::msg::InstantiateMsg;

use cosmwasm_std::{coins, to_binary, Addr, BlockInfo};
use croncat_sdk_factory::msg::{ContractMetadataResponse, ModuleInstantiateInfo, VersionKind};
use cw_multi_test::{App, AppBuilder, Executor};

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
            (u128::MAX, VERY_RICH.to_string()),
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

pub(crate) fn init_tasks(app: &mut App, msg: &InstantiateMsg, factory_addr: &Addr) -> Addr {
    let code_id = app.store_code(contracts::croncat_tasks_contract());
    let module_instantiate_info = ModuleInstantiateInfo {
        code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(msg).unwrap(),
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

pub(crate) fn init_manager(app: &mut App, factory_addr: &Addr) -> Addr {
    let code_id = app.store_code(contracts::croncat_manager_contract());
    let msg = croncat_manager::msg::InstantiateMsg {
        version: Some("0.1".to_owned()),
        croncat_tasks_key: ("tasks".to_owned(), [0, 1]),
        croncat_agents_key: ("agents".to_owned(), [0, 1]),
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        gas_price: None,
        treasury_addr: None,
        cw20_whitelist: None,
    };
    let module_instantiate_info = ModuleInstantiateInfo {
        code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&msg).unwrap(),
        contract_name: "manager".to_owned(),
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
                contract_name: "manager".to_owned(),
            },
        )
        .unwrap();
    metadata.metadata.unwrap().contract_addr
}

pub(crate) fn init_agents(app: &mut App, factory_addr: &Addr) -> Addr {
    let code_id = app.store_code(contracts::croncat_agents_contract());
    let msg = croncat_agents::msg::InstantiateMsg {
        version: Some("0.1".to_owned()),
        croncat_manager_key: ("manager".to_string(), [0, 1]),
        croncat_tasks_key: ("tasks".to_string(), [0, 1]),
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        agent_nomination_duration: None,
        min_tasks_per_agent: None,
        min_coins_for_agent_registration: None,
        agents_eject_threshold: None,
        min_active_agent_count: None,
        allowed_agents: Some(vec![]),
        public_registration: true,
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
                contract_name: "agents".to_owned(),
            },
        )
        .unwrap();
    metadata.metadata.unwrap().contract_addr
}

pub(crate) fn default_instantiate_msg() -> InstantiateMsg {
    InstantiateMsg {
        chain_name: "atom".to_owned(),
        version: Some("0.1".to_owned()),
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        croncat_manager_key: ("manager".to_owned(), [0, 1]),
        croncat_agents_key: ("agents".to_owned(), [0, 1]),
        slot_granularity_time: None,
        gas_base_fee: None,
        gas_action_fee: None,
        gas_query_fee: None,
        gas_limit: None,
    }
}

pub fn add_little_time(block: &mut BlockInfo) {
    block.time = block.time.plus_seconds(19);
    block.height += 1;
}
pub(crate) fn increment_block_height(block: &mut BlockInfo, inc_value: Option<u64>) {
    block.height += inc_value.unwrap_or(1);
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
