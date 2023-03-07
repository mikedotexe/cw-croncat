use crate::msg::*;
use crate::state::{
    DEFAULT_MIN_ACTIVE_AGENT_COUNT, DEFAULT_MIN_COINS_FOR_AGENT_REGISTRATION,
    DEFAULT_MIN_TASKS_PER_AGENT, DEFAULT_NOMINATION_BLOCK_DURATION,
};
use crate::tests::contracts;
use cosmwasm_std::BlockInfo;
use cosmwasm_std::{coins, to_binary, Addr};
use croncat_sdk_factory::msg::{
    ContractMetadataResponse, FactoryExecuteMsg, FactoryInstantiateMsg, ModuleInstantiateInfo,
    VersionKind,
};
use cw_multi_test::{App, AppBuilder, Executor};

pub const AGENT0: &str = "agent0a7uhnpqthunr2rzj0ww0hwurpn42wyun6c5puz";
pub const AGENT1: &str = "agent17muvdgkep4ndptnyg38eufxsssq8jr3wnkysy8";
pub const AGENT2: &str = "agent2qxywje86amll9ptzxmla5ah52uvsd9f7drs2dl";
pub const AGENT3: &str = "agent3c3cy3wzzz3698ypklvh7shksvmefj69xhm89z2";
pub const AGENT4: &str = "agent4ykfcyj8fl6xzs88tsls05x93gmq68a7km05m4j";
pub const AGENT5: &str = "agent5k5k7y4hgy5lkq0kj3k3e9k38lquh0m66kxsu5c";
pub const AGENT6: &str = "agent614a8clxc49z9e3mjzhamhkprt2hgf0y53zczzj0";

pub const AGENT_BENEFICIARY: &str = "cosmos1t5u0jfg3ljsjrh2m9e47d4ny2hea7eehxrzdgd";
pub const ADMIN: &str = "cosmos1sjllsnramtg3ewxqwwrwjxfgc4n4ef9u0tvx7u";
pub const PAUSE_ADMIN: &str = "cosmos338dwgj5wm2tuahvfjdldz5s8hmt7l5aznw8jz9s2mmgj5c52jqgfq000";

pub const ANYONE: &str = "cosmos1t5u0jfg3ljsjrh2m9e47d4ny2hea7eehxrzdgd";
pub const PARTICIPANT0: &str = "cosmos1055rfv3fv0zxsp8h3x88mctnm7x9mlgmf4m4d6";
pub const PARTICIPANT1: &str = "cosmos1c3cy3wzzz3698ypklvh7shksvmefj69xhm89z2";
pub const PARTICIPANT2: &str = "cosmos2far5cqkvny7k9wq53aw0k42v3f76rcylzzv05n";
pub const PARTICIPANT3: &str = "cosmos3xj3xagnprtqpfnvyp7k393kmes73rpuxqgamd8";
pub const PARTICIPANT4: &str = "cosmos4t5u0jfg3ljsjrh2m9e47d4ny2hea7eehxrzdgd";
pub const PARTICIPANT5: &str = "cosmos5t5u0jfg3ljsjrh2m9e47d4ny2hea7eehxrzdgd";
pub const PARTICIPANT6: &str = "cosmos6t5u0jfg3ljsjrh2m9e47d4ny2hea7eehxrzdgd";
pub const PARTICIPANT7: &str = "cosmos7t5u0jfg3ljsjrh2m9e47d4ny2hea7eehxrzdgd";

/// We set this to "TOKEN" to match the denom here:
/// https://github.com/CosmWasm/cosmwasm/blob/32f308a1a56ae5b8278947891306f7a374c3df94/packages/vm/src/environment.rs#L383
pub const NATIVE_DENOM: &str = "TOKEN";

#[allow(dead_code)]
pub(crate) struct TestScope {
    pub croncat_factory_addr: Addr,
    pub croncat_agents_addr: Addr,
    pub croncat_agents_code_id: Option<u64>,
    pub croncat_manager_addr: Addr,
    pub croncat_tasks_addr: Addr,
}

pub(crate) fn mock_config(croncat_factory_addr: &str) -> Config {
    Config {
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        owner_addr: Addr::unchecked(ADMIN),
        min_tasks_per_agent: DEFAULT_MIN_TASKS_PER_AGENT,
        agent_nomination_block_duration: DEFAULT_NOMINATION_BLOCK_DURATION,
        croncat_factory_addr: Addr::unchecked(croncat_factory_addr.to_owned()),
        croncat_manager_key: ("manager".to_owned(), [4, 2]),
        croncat_tasks_key: ("tasks".to_owned(), [42, 0]),
        min_coins_for_agent_registration: DEFAULT_MIN_COINS_FOR_AGENT_REGISTRATION,
        agents_eject_threshold: 600,
        min_active_agent_count: DEFAULT_MIN_ACTIVE_AGENT_COUNT,
        public_registration: true,
    }
}

pub(crate) fn mock_update_config(_croncat_factory_addr: &str) -> UpdateConfig {
    UpdateConfig {
        min_tasks_per_agent: Some(DEFAULT_MIN_TASKS_PER_AGENT),
        agent_nomination_duration: Some(DEFAULT_NOMINATION_BLOCK_DURATION),
        croncat_manager_key: Some(("manager".to_owned(), [4, 2])),
        croncat_tasks_key: Some(("tasks".to_owned(), [42, 0])),
        min_coins_for_agent_registration: None,
        agents_eject_threshold: None,
        min_active_agent_count: None,
        public_registration: Some(true),
    }
}

pub(crate) fn default_app() -> App {
    AppBuilder::new().build(|router, _, storage| {
        let accounts: Vec<(u128, String)> = vec![
            (6_000_000, ADMIN.to_string()),
            (500_000, ANYONE.to_string()),
            (500_000, AGENT0.to_string()),
            (500_000, AGENT1.to_string()),
            (500_000, AGENT2.to_string()),
            (500_000, AGENT3.to_string()),
            (500_000, AGENT4.to_string()),
            (500_000, AGENT5.to_string()),
            (500_000, AGENT6.to_string()),
        ];
        for (amt, address) in accounts {
            let coin = coins(amt, NATIVE_DENOM);
            router
                .bank
                .init_balance(storage, &Addr::unchecked(address), coin)
                .unwrap();
        }
    })
}

pub(crate) fn init_test_scope(app: &mut App) -> TestScope {
    let factory_code_id = app.store_code(contracts::croncat_factory_contract());
    let manager_code_id = app.store_code(contracts::croncat_manager_contract());
    let agents_code_id = app.store_code(contracts::croncat_agents_contract());
    let tasks_code_id = app.store_code(contracts::croncat_tasks_contract());

    let init_msg = FactoryInstantiateMsg {
        owner_addr: Some(ADMIN.to_owned()),
    };
    let croncat_factory_addr = app
        .instantiate_contract(
            factory_code_id,
            Addr::unchecked(ADMIN),
            &init_msg,
            &[],
            "factory",
            None,
        )
        .unwrap();

    // Manager
    let manager_module_instantiate_info = ModuleInstantiateInfo {
        code_id: manager_code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&croncat_manager::msg::InstantiateMsg {
            version: Some("0.1".to_owned()),
            croncat_tasks_key: ("tasks".to_owned(), [0, 1]),
            croncat_agents_key: ("agents".to_string(), [0, 1]),
            pause_admin: Addr::unchecked(PAUSE_ADMIN),
            gas_price: None,
            treasury_addr: None,
            cw20_whitelist: None,
        })
        .unwrap(),
        contract_name: "manager".to_owned(),
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        croncat_factory_addr.clone(),
        &FactoryExecuteMsg::Deploy {
            kind: VersionKind::Manager,
            module_instantiate_info: manager_module_instantiate_info,
        },
        &[],
    )
    .unwrap();

    let manager_contracts: ContractMetadataResponse = app
        .wrap()
        .query_wasm_smart(
            croncat_factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryQueryMsg::LatestContract {
                contract_name: "manager".to_string(),
            },
        )
        .unwrap();
    assert!(
        manager_contracts.metadata.is_some(),
        "Should be contract metadata"
    );
    let croncat_manager_addr = manager_contracts.metadata.unwrap().contract_addr;

    // Agents
    let agents_module_instantiate_info = ModuleInstantiateInfo {
        code_id: agents_code_id,
        version: [0, 1],
        commit_id: "some".to_owned(),
        checksum: "qwe123".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&croncat_agents::msg::InstantiateMsg {
            version: Some("0.1".to_owned()),
            croncat_manager_key: ("manager".to_owned(), [0, 1]),
            croncat_tasks_key: ("tasks".to_owned(), [0, 1]),
            pause_admin: Addr::unchecked(PAUSE_ADMIN),
            min_coins_for_agent_registration: None,
            agent_nomination_duration: None,
            min_tasks_per_agent: None,
            agents_eject_threshold: None,
            min_active_agent_count: None,
            allowed_agents: Some(vec![]),
            public_registration: true,
        })
        .unwrap(),
        contract_name: "agents".to_owned(),
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        croncat_factory_addr.clone(),
        &FactoryExecuteMsg::Deploy {
            kind: VersionKind::Agents,
            module_instantiate_info: agents_module_instantiate_info,
        },
        &[],
    )
    .unwrap();

    let agent_contracts: ContractMetadataResponse = app
        .wrap()
        .query_wasm_smart(
            croncat_factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryQueryMsg::LatestContract {
                contract_name: "agents".to_string(),
            },
        )
        .unwrap();
    assert!(
        agent_contracts.metadata.is_some(),
        "Should be contract metadata"
    );
    let agent_metadata = agent_contracts.metadata.unwrap();
    let croncat_agents_addr = agent_metadata.contract_addr;
    let croncat_agents_code_id = agent_metadata.code_id;

    // Tasks
    let tasks_module_instantiate_info = ModuleInstantiateInfo {
        code_id: tasks_code_id,
        version: [0, 1],
        commit_id: "some".to_owned(),
        checksum: "qwe123".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&croncat_tasks::msg::InstantiateMsg {
            chain_name: "cron".to_string(),
            version: Some("0.1".to_owned()),
            croncat_manager_key: ("manager".to_owned(), [0, 1]),
            croncat_agents_key: ("agents".to_string(), [0, 1]),
            slot_granularity_time: None,
            gas_base_fee: None,
            gas_action_fee: None,
            gas_query_fee: None,
            pause_admin: Addr::unchecked(PAUSE_ADMIN),
            gas_limit: None,
        })
        .unwrap(),
        contract_name: "tasks".to_owned(),
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        croncat_factory_addr.clone(),
        &FactoryExecuteMsg::Deploy {
            kind: VersionKind::Tasks,
            module_instantiate_info: tasks_module_instantiate_info,
        },
        &[],
    )
    .unwrap();

    let task_contracts: ContractMetadataResponse = app
        .wrap()
        .query_wasm_smart(
            croncat_factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryQueryMsg::LatestContract {
                contract_name: "tasks".to_string(),
            },
        )
        .unwrap();
    assert!(
        task_contracts.metadata.is_some(),
        "Should be contract metadata"
    );
    let croncat_tasks_addr = task_contracts.metadata.unwrap().contract_addr;

    TestScope {
        croncat_factory_addr,
        croncat_agents_addr,
        croncat_agents_code_id: Some(croncat_agents_code_id),
        croncat_manager_addr,
        croncat_tasks_addr,
    }
}

pub(crate) fn add_seconds_to_block(block: &mut BlockInfo, seconds: u64) {
    block.time = block.time.plus_seconds(seconds);
}

pub(crate) fn increment_block_height(block: &mut BlockInfo, inc_value: Option<u64>) {
    block.height += inc_value.unwrap_or(1);
}
