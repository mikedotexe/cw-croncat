use crate::tests::{
    common::add_seconds_to_block, helpers::increment_block_height, AGENT0, AGENT1, PARTICIPANT0,
    PARTICIPANT1, PAUSE_ADMIN,
};
use cosmwasm_std::{
    coin, coins, to_binary, Addr, BankMsg, Binary, StakingMsg, StdError, Timestamp, Uint128,
    Uint64, WasmMsg,
};
use croncat_sdk_core::types::{AmountForOneTask, GasPrice};
use croncat_sdk_factory::msg::{
    ContractMetadataResponse, FactoryExecuteMsg, ModuleInstantiateInfo, VersionKind,
};
use croncat_sdk_manager::{
    msg::ManagerExecuteMsg,
    types::{TaskBalance, TaskBalanceResponse},
};
use croncat_sdk_tasks::{
    msg::UpdateConfigMsg,
    types::{
        Action, Boundary, BoundaryHeight, BoundaryTime, Config, CroncatQuery,
        CurrentTaskInfoResponse, Interval, SlotHashesResponse, SlotTasksTotalResponse, Task,
        TaskInfo, TaskRequest, TaskResponse, Transform,
    },
};
use cw20::Cw20ExecuteMsg;
use cw_multi_test::{BankSudo, Executor};
use cw_storage_plus::KeyDeserialize;

use super::{
    contracts,
    helpers::{
        activate_agent, default_app, default_instantiate_msg, init_agents, init_factory,
        init_manager, init_mod_balances, init_tasks,
    },
    ADMIN, DENOM,
};
use crate::{
    contract::{GAS_ACTION_FEE, GAS_BASE_FEE, GAS_LIMIT, GAS_QUERY_FEE, SLOT_GRANULARITY_TIME},
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::TASKS_TOTAL,
    tests::{helpers::add_little_time, ANYONE},
    ContractError,
};

mod instantiate_tests {
    use crate::tests::PAUSE_ADMIN;

    use super::*;

    #[test]
    fn default_init() {
        let mut app = default_app();
        let factory_addr = init_factory(&mut app);

        let instantiate_msg: InstantiateMsg = default_instantiate_msg();
        let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
        let config: Config = app
            .wrap()
            .query_wasm_smart(tasks_addr, &QueryMsg::Config {})
            .unwrap();
        let expected_config = Config {
            version: "0.1".to_owned(),
            owner_addr: factory_addr.clone(),
            pause_admin: Addr::unchecked(PAUSE_ADMIN),
            croncat_factory_addr: factory_addr,
            chain_name: "atom".to_owned(),
            croncat_manager_key: ("manager".to_owned(), [0, 1]),
            croncat_agents_key: ("agents".to_owned(), [0, 1]),
            slot_granularity_time: SLOT_GRANULARITY_TIME,
            gas_base_fee: GAS_BASE_FEE,
            gas_action_fee: GAS_ACTION_FEE,
            gas_query_fee: GAS_QUERY_FEE,
            gas_limit: GAS_LIMIT,
        };

        assert_eq!(config, expected_config);
    }

    #[test]
    fn custom_init() {
        let mut app = default_app();
        let factory_addr = init_factory(&mut app);

        let instantiate_msg: InstantiateMsg = InstantiateMsg {
            chain_name: "cron".to_owned(),
            version: Some("0.1".to_owned()),
            pause_admin: Addr::unchecked(PAUSE_ADMIN),
            croncat_manager_key: ("definitely_not_manager".to_owned(), [4, 2]),
            croncat_agents_key: ("definitely_not_agents".to_owned(), [42, 0]),
            slot_granularity_time: Some(10),
            gas_base_fee: Some(1),
            gas_action_fee: Some(2),
            gas_query_fee: Some(3),
            gas_limit: Some(10),
        };
        let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
        let config: Config = app
            .wrap()
            .query_wasm_smart(tasks_addr, &QueryMsg::Config {})
            .unwrap();

        let expected_config = Config {
            version: "0.1".to_owned(),
            owner_addr: factory_addr.clone(),
            pause_admin: Addr::unchecked(PAUSE_ADMIN),
            croncat_factory_addr: factory_addr,
            chain_name: "cron".to_owned(),
            croncat_manager_key: ("definitely_not_manager".to_owned(), [4, 2]),
            croncat_agents_key: ("definitely_not_agents".to_owned(), [42, 0]),
            slot_granularity_time: 10,
            gas_base_fee: 1,
            gas_action_fee: 2,
            gas_query_fee: 3,
            gas_limit: 10,
        };
        assert_eq!(config, expected_config);
    }

    #[test]
    fn failed_inits() {
        let mut app = default_app();
        let code_id = app.store_code(contracts::croncat_tasks_contract());

        let instantiate_msg: InstantiateMsg = InstantiateMsg {
            pause_admin: Addr::unchecked("InVA$LID_ADDR"),
            ..default_instantiate_msg()
        };
        let contract_err: ContractError = app
            .instantiate_contract(
                code_id,
                Addr::unchecked(ADMIN),
                &instantiate_msg,
                &[],
                "tasks",
                None,
            )
            .unwrap_err()
            .downcast()
            .unwrap();

        assert_eq!(
            contract_err,
            ContractError::Std(StdError::generic_err(
                "Invalid input: address not normalized"
            ))
        );
    }
}

#[test]
fn create_task_without_query() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let manager_addr = init_manager(&mut app, &factory_addr);
    let _ = init_agents(&mut app, &factory_addr);

    let action1 = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(50_000),
    };

    let action2 = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT0).to_string(),
            amount: coins(10, DENOM),
        }
        .into(),
        gas_limit: Some(100_000),
    };

    let task = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some((app.block_info().height).into()),
            end: Some((app.block_info().height + 10).into()),
        })),
        stop_on_fail: false,
        actions: vec![action1.clone(), action2.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap();

    // check it created task with responded task hash and can be queried from anywhere
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();
    assert!(task_hash.starts_with("atom:"));
    let tasks: Vec<TaskInfo> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::Tasks {
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();
    assert_eq!(task_response.task.clone().unwrap(), tasks[0]);
    let expected_block_task_response = TaskResponse {
        task: Some(TaskInfo {
            task_hash: task_hash.clone(),
            owner_addr: Addr::unchecked(ANYONE),
            interval: Interval::Once,
            boundary: Boundary::Height(BoundaryHeight {
                start: Some(app.block_info().height.into()),
                end: Some((app.block_info().height + 10).into()),
            }),
            stop_on_fail: false,
            amount_for_one_task: AmountForOneTask {
                cw20: None,
                coin: [Some(coin(15, DENOM)), None],
                gas: GAS_BASE_FEE + action1.gas_limit.unwrap() + action2.gas_limit.unwrap(),
                agent_fee: 5,
                treasury_fee: 5,
                gas_price: GasPrice {
                    numerator: 4,
                    denominator: 100,
                    gas_adjustment_numerator: 150,
                },
            },
            actions: vec![action1, action2],
            queries: None,
            transforms: vec![],
            version: "0.1".to_owned(),
        }),
    };
    assert_eq!(task_response.task, expected_block_task_response.task);

    // check total tasks
    let total_tasks: Uint64 = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::TasksTotal {})
        .unwrap();
    assert_eq!(total_tasks, Uint64::new(1));

    // check it created balance on the manager contract
    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &croncat_manager::msg::QueryMsg::TaskBalance { task_hash },
        )
        .unwrap();
    assert_eq!(
        manager_task_balance.balance,
        Some(TaskBalance {
            native_balance: Uint128::new(30000),
            cw20_balance: None,
            ibc_balance: None,
        }),
    );

    // Check it's next item
    let current_slot: TaskResponse = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::CurrentTask {})
        .unwrap();
    assert!(current_slot.task.is_none());
    app.update_block(add_little_time);
    let slot_total: SlotTasksTotalResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::SlotTasksTotal { offset: None },
        )
        .unwrap();
    assert_eq!(
        slot_total,
        SlotTasksTotalResponse {
            block_tasks: 1,
            cron_tasks: 0,
            evented_tasks: 0,
        }
    );
    let current_slot: TaskResponse = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::CurrentTask {})
        .unwrap();

    assert_eq!(current_slot.task, expected_block_task_response.task);

    // check it all transferred out of tasks
    let manager_balance = app
        .wrap()
        .query_balance(manager_addr.clone(), DENOM)
        .unwrap();
    let tasks_balance = app.wrap().query_balance(tasks_addr.clone(), DENOM).unwrap();
    assert_eq!(manager_balance, coin(30000, DENOM));
    assert_eq!(tasks_balance, coin(0, DENOM));

    // Create second task do same checks, but add second coin
    app.sudo(
        BankSudo::Mint {
            to_address: ANYONE.to_owned(),
            amount: coins(10, "test_coins"),
        }
        .into(),
    )
    .unwrap();
    let action = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: vec![coin(10, DENOM), coin(5, "test_coins")],
        }
        .into(),
        gas_limit: Some(60_000),
    };
    let task = TaskRequest {
        interval: Interval::Cron("* * * * * *".to_owned()),
        boundary: Some(Boundary::Time(BoundaryTime {
            start: Some(app.block_info().time),
            end: Some(app.block_info().time.plus_nanos(100)),
        })),
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &[coin(60000, DENOM), coin(10, "test_coins")],
        )
        .unwrap();

    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();
    assert!(task_hash.starts_with("atom:"));
    let responses: Vec<TaskInfo> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::TasksByOwner {
                owner_addr: ANYONE.to_owned(),
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    let task_from_task_list = responses
        .into_iter()
        .find(|task_res| task_res.clone().task_hash == task_hash)
        .unwrap();
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();
    assert_eq!(task_response.task.clone().unwrap(), task_from_task_list);

    let expected_cron_task_response = TaskResponse {
        task: Some(TaskInfo {
            task_hash: task_hash.clone(),
            owner_addr: Addr::unchecked(ANYONE),
            interval: Interval::Cron("* * * * * *".to_owned()),
            boundary: Boundary::Time(BoundaryTime {
                start: Some(app.block_info().time),
                end: Some(app.block_info().time.plus_nanos(100)),
            }),
            stop_on_fail: false,
            amount_for_one_task: AmountForOneTask {
                cw20: None,
                coin: [Some(coin(10, DENOM)), Some(coin(5, "test_coins"))],
                gas: GAS_BASE_FEE + action.gas_limit.unwrap(),
                agent_fee: 5,
                treasury_fee: 5,
                gas_price: GasPrice {
                    numerator: 4,
                    denominator: 100,
                    gas_adjustment_numerator: 150,
                },
            },
            actions: vec![action],
            queries: None,
            transforms: vec![],
            version: "0.1".to_owned(),
        }),
    };
    assert_eq!(task_response.task, expected_cron_task_response.task);

    let total_tasks: Uint64 = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::TasksTotal {})
        .unwrap();
    assert_eq!(total_tasks, Uint64::new(2));
    // Check that tasks total compares
    let total_t = TASKS_TOTAL.query(&app.wrap(), tasks_addr.clone()).unwrap();
    assert_eq!(total_t, 2);

    // Check it got queued into correct slot
    app.update_block(add_little_time);
    let slot_total: SlotTasksTotalResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::SlotTasksTotal { offset: None },
        )
        .unwrap();
    assert_eq!(
        slot_total,
        SlotTasksTotalResponse {
            block_tasks: 1,
            cron_tasks: 1,
            evented_tasks: 0,
        }
    );

    // Check it prefers block over cron
    let current_slot: TaskResponse = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::CurrentTask {})
        .unwrap();
    assert_eq!(current_slot.task, expected_block_task_response.task);

    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &croncat_manager::msg::QueryMsg::TaskBalance { task_hash },
        )
        .unwrap();
    assert_eq!(
        manager_task_balance.balance,
        Some(TaskBalance {
            native_balance: Uint128::new(60000),
            cw20_balance: None,
            ibc_balance: Some(coin(10, "test_coins")),
        }),
    );

    let manager_balance = app.wrap().query_all_balances(manager_addr).unwrap();
    let tasks_balance = app.wrap().query_all_balances(tasks_addr).unwrap();
    assert_eq!(
        manager_balance,
        vec![coin(30000 + 60000, DENOM), coin(10, "test_coins")]
    );
    assert_eq!(tasks_balance, vec![]);
}

#[test]
fn check_task_timestamp() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let _ = init_manager(&mut app, &factory_addr);
    let _ = init_agents(&mut app, &factory_addr);

    let action1 = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(50_000),
    };

    let action2 = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT0).to_string(),
            amount: coins(10, DENOM),
        }
        .into(),
        gas_limit: Some(100_000),
    };

    let mut task = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some((app.block_info().height).into()),
            end: Some((app.block_info().height + 10).into()),
        })),
        stop_on_fail: false,
        actions: vec![action1, action2],
        queries: None,
        transforms: None,
        cw20: None,
    };
    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &ExecuteMsg::CreateTask {
            task: Box::new(task.clone()),
        },
        &coins(30000, DENOM),
    )
    .expect("Couldn't create first task");

    // Check latest timestamp
    let mut latest_timestamp: CurrentTaskInfoResponse = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::CurrentTaskInfo {})
        .unwrap();
    let mut expected = CurrentTaskInfoResponse {
        total: Uint64::one(),
        last_created_task: Timestamp::from_nanos(1571797419879305533),
    };
    assert_eq!(latest_timestamp, expected);
    app.update_block(|block| add_seconds_to_block(block, 666));

    // Another task
    task.boundary = None; // Change a detail to avoid task hash collision

    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &ExecuteMsg::CreateTask {
            task: Box::new(task),
        },
        &coins(30000, DENOM),
    )
    .expect("Couldn't create second task");
    latest_timestamp = app
        .wrap()
        .query_wasm_smart(tasks_addr, &QueryMsg::CurrentTaskInfo {})
        .unwrap();
    expected.last_created_task = Timestamp::from_nanos(1571798085879305533);
    expected.total = Uint64::new(2u64);
    assert_eq!(latest_timestamp, expected);

    // Note: At the time of this writing, we've discussed at length whether removal of a task should change the latest task creation time. We've decided it's not needed for the beta release.
    // Tracked here: https://github.com/CronCats/cw-croncat/issues/319
}

#[test]
fn create_task_with_wasm() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let manager_addr = init_manager(&mut app, &factory_addr);
    let agents_addr = init_agents(&mut app, &factory_addr);

    // let action = Action {
    //     msg: WasmMsg::Execute {
    //         contract_addr: agents_addr.to_string(),
    //         msg: to_binary(&croncat_sdk_agents::msg::ExecuteMsg::Tick {}).unwrap(),
    //         funds: vec![],
    //     }
    //     .into(),
    //     gas_limit: Some(150_000),
    // };
    let action = Action {
        msg: WasmMsg::Execute {
            contract_addr: agents_addr.to_string(),
            msg: to_binary(&croncat_sdk_agents::msg::ExecuteMsg::Tick {}).unwrap(),
            funds: vec![],
        }
        .into(),
        gas_limit: Some(150_000),
    };

    let task = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some((app.block_info().height).into()),
            end: Some((app.block_info().height + 10).into()),
        })),
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        factory_addr.clone(),
        &croncat_sdk_factory::msg::FactoryExecuteMsg::Proxy {
            msg: WasmMsg::Execute {
                contract_addr: tasks_addr.to_string(),
                msg: to_binary(&ExecuteMsg::CreateTask {
                    task: Box::new(task),
                })
                .unwrap(),
                funds: coins(30000, DENOM),
            },
        },
        &coins(30000, DENOM),
    )
    .unwrap();
    // let task_hash = String::from_vec(res.data.unwrap().0).unwrap();

    // check total tasks
    let total_tasks: Uint64 = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::TasksTotal {})
        .unwrap();
    assert_eq!(total_tasks, Uint64::new(1));

    let owner_tasks: Vec<TaskInfo> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &QueryMsg::TasksByOwner {
                owner_addr: factory_addr.clone().to_string(),
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    let task_hash = &owner_tasks[0].task_hash;

    // check it created balance on the manager contract
    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr,
            &croncat_manager::msg::QueryMsg::TaskBalance {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        manager_task_balance.balance,
        Some(TaskBalance {
            native_balance: Uint128::new(30000),
            cw20_balance: None,
            ibc_balance: None,
        }),
    );
}

#[test]
fn create_tasks_with_queries_and_transforms() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let manager_addr = init_manager(&mut app, &factory_addr);
    let _ = init_agents(&mut app, &factory_addr);

    let action = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(50_000),
    };
    let queries = vec![
        CroncatQuery {
            contract_addr: "aloha123".to_owned(),
            msg: Binary::from([4, 2]),
            check_result: true,
        },
        CroncatQuery {
            contract_addr: "aloha321".to_owned(),
            msg: Binary::from([2, 4]),
            check_result: true,
        },
    ];
    let transforms = vec![Transform {
        action_idx: 1,
        query_idx: 2,
        action_path: vec![5u64.into()].into(),
        query_response_path: vec![5u64.into()].into(),
    }];

    let task = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some((app.block_info().height).into()),
            end: Some((app.block_info().height + 10).into()),
        })),
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: Some(queries.clone()),
        transforms: Some(transforms.clone()),
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(50000, DENOM),
        )
        .unwrap();
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();
    let tasks: Vec<TaskInfo> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::Tasks {
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();
    assert_eq!(task_response.task.clone().unwrap(), tasks[0]);

    let expected_block_task_response = TaskResponse {
        task: Some(TaskInfo {
            task_hash: task_hash.clone(),
            owner_addr: Addr::unchecked(ANYONE),
            interval: Interval::Once,
            boundary: Boundary::Height(BoundaryHeight {
                start: Some(app.block_info().height.into()),
                end: Some((app.block_info().height + 10).into()),
            }),
            stop_on_fail: false,
            amount_for_one_task: AmountForOneTask {
                cw20: None,
                coin: [Some(coin(5, DENOM)), None],
                gas: GAS_BASE_FEE + action.gas_limit.unwrap() + GAS_QUERY_FEE * 2,
                agent_fee: 5,
                treasury_fee: 5,
                gas_price: GasPrice {
                    numerator: 4,
                    denominator: 100,
                    gas_adjustment_numerator: 150,
                },
            },
            actions: vec![action],
            queries: Some(queries),
            transforms,
            version: "0.1".to_owned(),
        }),
    };
    assert_eq!(task_response.task, expected_block_task_response.task);

    let total_tasks: Uint64 = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::TasksTotal {})
        .unwrap();
    assert_eq!(total_tasks, Uint64::new(1));

    // check it created balance on the manager contract
    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &croncat_manager::msg::QueryMsg::TaskBalance { task_hash },
        )
        .unwrap();
    assert_eq!(
        manager_task_balance.balance,
        Some(TaskBalance {
            native_balance: Uint128::new(50000),
            cw20_balance: None,
            ibc_balance: None,
        }),
    );

    // should not be scheduled
    app.update_block(add_little_time);
    let slot_total: SlotTasksTotalResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::SlotTasksTotal { offset: None },
        )
        .unwrap();
    assert_eq!(
        slot_total,
        SlotTasksTotalResponse {
            block_tasks: 0,
            cron_tasks: 0,
            evented_tasks: 1,
        }
    );

    let manager_balance = app.wrap().query_all_balances(manager_addr).unwrap();
    let tasks_balance = app.wrap().query_all_balances(tasks_addr).unwrap();
    assert_eq!(manager_balance, vec![coin(50000, DENOM)]);
    assert_eq!(tasks_balance, vec![]);
}

#[test]
fn remove_tasks_fail() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let _ = init_manager(&mut app, &factory_addr);
    let _ = init_agents(&mut app, &factory_addr);

    // Try RemoveTask with wrong hash
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::RemoveTask {
                task_hash: "hash".to_string(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NoTaskFound {});

    // Create two tasks, one with queries, another without queries
    // With query:
    let task_with_query = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: Addr::unchecked(PARTICIPANT1).to_string(),
                amount: coins(5, DENOM),
            }
            .into(),
            gas_limit: Some(50_000),
        }],
        queries: Some(vec![
            CroncatQuery {
                contract_addr: "aloha123".to_owned(),
                msg: Binary::from([4, 2]),
                check_result: true,
            },
            CroncatQuery {
                contract_addr: "aloha321".to_owned(),
                msg: Binary::from([2, 4]),
                check_result: true,
            },
        ]),
        transforms: Some(vec![Transform {
            action_idx: 1,
            query_idx: 2,
            action_path: vec![5u64.into()].into(),
            query_response_path: vec![5u64.into()].into(),
        }]),
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task_with_query),
            },
            &coins(50000, DENOM),
        )
        .unwrap();
    let task_hash_with_queries = String::from_vec(res.data.unwrap().0).unwrap();

    // Without queries
    let task_without_query = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: Addr::unchecked(PARTICIPANT1).to_string(),
                amount: coins(5, DENOM),
            }
            .into(),
            gas_limit: Some(50_000),
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task_without_query),
            },
            &coins(50000, DENOM),
        )
        .unwrap();
    let task_hash_without_queries = String::from_vec(res.data.unwrap().0).unwrap();

    // Another user tries to remove the task
    // With queries:
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            tasks_addr.clone(),
            &ExecuteMsg::RemoveTask {
                task_hash: task_hash_with_queries.to_owned(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    // Without queries
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            tasks_addr.clone(),
            &ExecuteMsg::RemoveTask {
                task_hash: task_hash_without_queries.to_owned(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    // Pause admin should be able to pause
    let res = app.execute_contract(
        Addr::unchecked(PAUSE_ADMIN),
        tasks_addr.clone(),
        &ExecuteMsg::PauseContract {},
        &[],
    );
    assert!(res.is_ok());

    // With queries:
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::RemoveTask {
                task_hash: task_hash_with_queries,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::ContractPaused {});

    // Without queries
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr,
            &ExecuteMsg::RemoveTask {
                task_hash: task_hash_without_queries,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::ContractPaused {});
}

#[test]
fn remove_tasks_with_queries_success() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let manager_addr = init_manager(&mut app, &factory_addr);
    let _ = init_agents(&mut app, &factory_addr);

    // Create one block and one cron with queries and then remove one by one
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some((app.block_info().height + 10).into()),
            end: Some((app.block_info().height + 20).into()),
        })),
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: Addr::unchecked(PARTICIPANT1).to_string(),
                amount: coins(5, DENOM),
            }
            .into(),
            gas_limit: Some(50_000),
        }],
        queries: Some(vec![
            CroncatQuery {
                contract_addr: "aloha123".to_owned(),
                msg: Binary::from([4, 2]),
                check_result: true,
            },
            CroncatQuery {
                contract_addr: "aloha321".to_owned(),
                msg: Binary::from([2, 4]),
                check_result: true,
            },
        ]),
        transforms: Some(vec![Transform {
            action_idx: 1,
            query_idx: 2,
            action_path: vec![5u64.into()].into(),
            query_response_path: vec![5u64.into()].into(),
        }]),
        cw20: None,
    };

    let task_raw = Task {
        owner_addr: Addr::unchecked(ANYONE),
        interval: task.interval.clone(),
        boundary: Boundary::Height(BoundaryHeight {
            start: Some(Uint64::new(app.block_info().height)),
            end: None,
        }),
        stop_on_fail: task.stop_on_fail,
        actions: task.actions.clone(),
        queries: task.queries.clone().unwrap(),
        transforms: task.transforms.clone().unwrap(),
        version: "0.1".to_string(),
        amount_for_one_task: AmountForOneTask {
            cw20: None,
            coin: [Some(coin(5, DENOM)), None],
            gas: 50_000,
            agent_fee: u16::default(),
            treasury_fee: u16::default(),
            gas_price: GasPrice::default(),
        },
    };
    assert!(task_raw.is_evented());
    assert!(task_raw.is_evented() && task_raw.boundary.is_block());

    let task_raw_non_evented = Task {
        owner_addr: Addr::unchecked(ANYONE),
        interval: task.interval.clone(),
        boundary: Boundary::Height(BoundaryHeight {
            start: Some(Uint64::new(app.block_info().height + 10)),
            end: None,
        }),
        stop_on_fail: task.stop_on_fail,
        actions: task.actions.clone(),
        queries: vec![CroncatQuery {
            contract_addr: "aloha321".to_owned(),
            msg: Binary::from([2, 4]),
            check_result: false,
        }],
        transforms: task.transforms.clone().unwrap(),
        version: "0.1".to_string(),
        amount_for_one_task: AmountForOneTask {
            cw20: None,
            coin: [Some(coin(5, DENOM)), None],
            gas: 50_000,
            agent_fee: u16::default(),
            treasury_fee: u16::default(),
            gas_price: GasPrice::default(),
        },
    };
    assert!(!task_raw_non_evented.is_evented());
    assert!(!task_raw_non_evented.is_evented() && task_raw.boundary.is_block());

    // test how the index will find it
    let v = match task_raw_non_evented.boundary {
        Boundary::Height(h) => h.start.unwrap_or(Uint64::zero()).into(),
        Boundary::Time(t) => {
            if let Some(t) = t.start {
                t.nanos()
            } else {
                u64::default()
            }
        }
    };
    assert_eq!(v, app.block_info().height + 10);

    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(50000, DENOM),
        )
        .unwrap();
    let task_hash_block_with_queries = String::from_vec(res.data.unwrap().0).unwrap();

    // check it created balance on the manager contract
    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &croncat_manager::msg::QueryMsg::TaskBalance {
                task_hash: task_hash_block_with_queries.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        manager_task_balance.balance,
        Some(TaskBalance {
            native_balance: Uint128::new(50000),
            cw20_balance: None,
            ibc_balance: None,
        }),
    );

    let task = TaskRequest {
        interval: Interval::Cron("* * * * * *".to_owned()),
        boundary: Some(Boundary::Time(BoundaryTime {
            start: Some(app.block_info().time.plus_nanos(10000)),
            end: Some(app.block_info().time.plus_nanos(20000)),
        })),
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: Addr::unchecked(PARTICIPANT1).to_string(),
                amount: coins(5, DENOM),
            }
            .into(),
            gas_limit: Some(50_000),
        }],
        queries: Some(vec![
            CroncatQuery {
                contract_addr: "aloha123".to_owned(),
                msg: Binary::from([4, 2]),
                check_result: true,
            },
            CroncatQuery {
                contract_addr: "aloha321".to_owned(),
                msg: Binary::from([2, 4]),
                check_result: true,
            },
        ]),
        transforms: Some(vec![Transform {
            action_idx: 1,
            query_idx: 2,
            action_path: vec![5u64.into()].into(),
            query_response_path: vec![5u64.into()].into(),
        }]),
        cw20: None,
    };

    let task_no_evented = TaskRequest {
        interval: Interval::Cron("* * * * * *".to_owned()),
        boundary: Some(Boundary::Time(BoundaryTime {
            start: Some(app.block_info().time.plus_nanos(10000)),
            end: Some(app.block_info().time.plus_nanos(20000)),
        })),
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: Addr::unchecked(PARTICIPANT1).to_string(),
                amount: coins(5, DENOM),
            }
            .into(),
            gas_limit: Some(50_000),
        }],
        queries: Some(vec![
            CroncatQuery {
                contract_addr: "aloha123".to_owned(),
                msg: Binary::from([4, 2]),
                check_result: false,
            },
            CroncatQuery {
                contract_addr: "aloha321".to_owned(),
                msg: Binary::from([2, 4]),
                check_result: false,
            },
        ]),
        transforms: Some(vec![Transform {
            action_idx: 1,
            query_idx: 2,
            action_path: vec![5u64.into()].into(),
            query_response_path: vec![5u64.into()].into(),
        }]),
        cw20: None,
    };

    // Make sure to test evented task with Cron interval doesnt work
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(90000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidInterval {});

    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task_no_evented),
            },
            &coins(90000, DENOM),
        )
        .unwrap();
    let task_hash_cron_with_queries_evented = String::from_vec(res.data.unwrap().0).unwrap();

    let evented_ids: Vec<u64> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::EventedIds {
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(evented_ids.len(), 1);
    assert_eq!(evented_ids, [12355]);

    let evented_hashes: Vec<String> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::EventedHashes {
                id: None,
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(evented_hashes.len(), 1);
    let evented_hashes: Vec<String> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::EventedHashes {
                id: Some(app.block_info().height + 10),
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(evented_hashes.len(), 1);
    let evented_hashes: Vec<String> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::EventedHashes {
                // id: Some(app.block_info().time.nanos()),
                id: Some(12355),
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(evented_hashes.len(), 1);

    let evented_task_response_any: Vec<Option<TaskInfo>> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::EventedTasks {
                start: None,
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    let evented_task_response_start_block: Vec<Option<TaskInfo>> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::EventedTasks {
                start: Some(app.block_info().height + 10),
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    let evented_task_response_start_time: Vec<Option<TaskInfo>> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::EventedTasks {
                // id: Some(app.block_info().time.nanos()),
                start: Some(1571797410000000000),
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    // Check respone amounts!
    assert_eq!(evented_task_response_any.len(), 1);
    assert_eq!(evented_task_response_start_block.len(), 1);
    assert_eq!(evented_task_response_start_time.len(), 0);

    // remove block task
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::RemoveTask {
                task_hash: task_hash_block_with_queries.clone(),
            },
            &[],
        )
        .unwrap();

    // Check attributes
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "action" && attr.value == "remove_task")
    }));

    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::Task {
                task_hash: task_hash_block_with_queries.clone(),
            },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // check it removed balance on the manager contract
    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &croncat_manager::msg::QueryMsg::TaskBalance {
                task_hash: task_hash_block_with_queries,
            },
        )
        .unwrap();
    assert!(manager_task_balance.balance.is_none());

    // remove evented cron task
    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr,
        &ExecuteMsg::RemoveTask {
            task_hash: task_hash_cron_with_queries_evented.clone(),
        },
        &[],
    )
    .unwrap();

    // check it removed balance on the manager contract
    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &croncat_manager::msg::QueryMsg::TaskBalance {
                task_hash: task_hash_cron_with_queries_evented,
            },
        )
        .unwrap();
    assert!(manager_task_balance.balance.is_none());

    // Check all balances moved from manager contract
    let manager_balance = app.wrap().query_all_balances(manager_addr).unwrap();
    assert!(manager_balance.is_empty());
}

#[test]
fn remove_tasks_without_queries_success() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let manager_addr = init_manager(&mut app, &factory_addr);
    let _ = init_agents(&mut app, &factory_addr);

    // Create one block and one cron without queries and then remove one by one
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some((app.block_info().height).into()),
            end: Some((app.block_info().height + 10).into()),
        })),
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: Addr::unchecked(PARTICIPANT1).to_string(),
                amount: coins(5, DENOM),
            }
            .into(),
            gas_limit: Some(50_000),
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(50000, DENOM),
        )
        .unwrap();
    let task_hash_block_without_queries = String::from_vec(res.data.unwrap().0).unwrap();

    // check it created balance on the manager contract
    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &croncat_manager::msg::QueryMsg::TaskBalance {
                task_hash: task_hash_block_without_queries.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        manager_task_balance.balance,
        Some(TaskBalance {
            native_balance: Uint128::new(50000),
            cw20_balance: None,
            ibc_balance: None,
        }),
    );

    let task = TaskRequest {
        interval: Interval::Cron("* * * * * *".to_owned()),
        boundary: Some(Boundary::Time(BoundaryTime {
            start: Some(app.block_info().time),
            end: Some(app.block_info().time.plus_nanos(1000)),
        })),
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: Addr::unchecked(PARTICIPANT1).to_string(),
                amount: coins(5, DENOM),
            }
            .into(),
            gas_limit: Some(50_000),
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };

    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(50000, DENOM),
        )
        .unwrap();
    let task_hash_cron_without_queries = String::from_vec(res.data.unwrap().0).unwrap();

    // check it created balance on the manager contract
    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &croncat_manager::msg::QueryMsg::TaskBalance {
                task_hash: task_hash_cron_without_queries.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        manager_task_balance.balance,
        Some(TaskBalance {
            native_balance: Uint128::new(50000),
            cw20_balance: None,
            ibc_balance: None,
        }),
    );

    // remove block task
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::RemoveTask {
                task_hash: task_hash_block_without_queries.clone(),
            },
            &[],
        )
        .unwrap();

    // Check attributes
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "action" && attr.value == "remove_task")
    }));

    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::Task {
                task_hash: task_hash_block_without_queries.clone(),
            },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // check it removed balance on the manager contract
    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &croncat_manager::msg::QueryMsg::TaskBalance {
                task_hash: task_hash_block_without_queries,
            },
        )
        .unwrap();
    assert!(manager_task_balance.balance.is_none());

    // remove cron task
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::RemoveTask {
                task_hash: task_hash_cron_without_queries.clone(),
            },
            &[],
        )
        .unwrap();

    // Check attributes
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "action" && attr.value == "remove_task")
    }));

    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &QueryMsg::Task {
                task_hash: task_hash_cron_without_queries.clone(),
            },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // check it removed balance on the manager contract
    let manager_task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &croncat_manager::msg::QueryMsg::TaskBalance {
                task_hash: task_hash_cron_without_queries,
            },
        )
        .unwrap();
    assert!(manager_task_balance.balance.is_none());

    // Check all balances moved from manager contract
    let manager_balance = app.wrap().query_all_balances(manager_addr).unwrap();
    assert!(manager_balance.is_empty());
}

#[test]
fn update_cfg() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);

    // Call Factory contract's Proxy method, telling it to update Tasks Config and pause the contract
    let msg = WasmMsg::Execute {
        contract_addr: tasks_addr.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateConfig(UpdateConfigMsg {
            croncat_factory_addr: Some("fixed_croncat_factory_addr".to_owned()),
            croncat_manager_key: Some(("new_manager2".to_owned(), [2, 2])),
            croncat_agents_key: Some(("new_agents2".to_owned(), [2, 2])),
            slot_granularity_time: Some(54),
            gas_base_fee: Some(1),
            gas_action_fee: Some(2),
            gas_query_fee: Some(3),
            gas_limit: Some(42),
        }))
        .unwrap(),
        funds: vec![],
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        factory_addr.clone(),
        &FactoryExecuteMsg::Proxy { msg },
        &[],
    )
    .unwrap();

    let config: Config = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    let expected_config = Config {
        version: "0.1".to_owned(),
        owner_addr: factory_addr.clone(),
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        croncat_factory_addr: Addr::unchecked("fixed_croncat_factory_addr"),
        chain_name: "atom".to_owned(),
        croncat_manager_key: ("new_manager2".to_owned(), [2, 2]),
        croncat_agents_key: ("new_agents2".to_owned(), [2, 2]),
        slot_granularity_time: 54,
        gas_base_fee: 1,
        gas_action_fee: 2,
        gas_query_fee: 3,
        gas_limit: 42,
    };

    assert_eq!(config, expected_config);

    // None's shouldn't impact any of the fields
    // Call Factory contract's Proxy method, telling it to update Tasks Config and pause the contract
    let msg = WasmMsg::Execute {
        contract_addr: tasks_addr.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateConfig(UpdateConfigMsg {
            croncat_factory_addr: None,
            croncat_manager_key: None,
            croncat_agents_key: None,
            slot_granularity_time: None,
            gas_base_fee: None,
            gas_action_fee: None,
            gas_query_fee: None,
            gas_limit: None,
        }))
        .unwrap(),
        funds: vec![],
    };
    app.execute_contract(
        // It was recently changed owners, remember
        Addr::unchecked(ADMIN),
        factory_addr.clone(),
        &FactoryExecuteMsg::Proxy { msg },
        &[],
    )
    .unwrap();

    let not_updated_config: Config = app
        .wrap()
        .query_wasm_smart(tasks_addr, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(not_updated_config, expected_config);
}

#[test]
fn negative_create_task() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let _ = init_manager(&mut app, &factory_addr);
    let _ = init_agents(&mut app, &factory_addr);

    let action = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(50_000),
    };

    let task = TaskRequest {
        interval: Interval::Cron("aloha".to_string()),
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        transforms: None,
        cw20: None,
        queries: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidInterval {});
    // invalid gas limit
    let action1 = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(GAS_LIMIT / 2),
    };
    let action2 = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT0).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(GAS_LIMIT / 2 + 1),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some((app.block_info().height).into()),
            end: Some((app.block_info().height + 10).into()),
        })),
        stop_on_fail: false,
        actions: vec![action1, action2],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidGas {});

    // Invalid boundary
    let action = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(25_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some((app.block_info().height).into()),
            end: Some((app.block_info().height).into()),
        })),
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidBoundary {});

    // Same task - can't repeat tasks
    let action = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(25_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &ExecuteMsg::CreateTask {
            task: Box::new(task.clone()),
        },
        &coins(30000, DENOM),
    )
    .unwrap();
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::TaskExists {});

    // Same task, but with queries - can't repeat tasks
    let action = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(25_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: Some(vec![CroncatQuery {
            contract_addr: "aloha".to_owned(),
            msg: Default::default(),
            check_result: true,
        }]),
        transforms: None,
        cw20: None,
    };
    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &ExecuteMsg::CreateTask {
            task: Box::new(task.clone()),
        },
        &coins(40000, DENOM),
    )
    .unwrap();
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(40000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::TaskExists {});
    // contract paused

    // Pause admin should be able to pause
    let res = app.execute_contract(
        Addr::unchecked(PAUSE_ADMIN),
        tasks_addr.clone(),
        &ExecuteMsg::PauseContract {},
        &[],
    );
    assert!(res.is_ok());

    let action = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(25_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr,
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::ContractPaused {});
}

#[test]
fn remove_task_negative() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let _manager_addr = init_manager(&mut app, &factory_addr);
    let _agent_addr = init_agents(&mut app, &factory_addr);

    let action = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT1).to_string(),
            amount: coins(5, DENOM),
        }
        .into(),
        gas_limit: Some(25_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(40000, DENOM),
        )
        .unwrap();

    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();

    // Not task owner tries to remove a task
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("wrong_person"),
            tasks_addr.clone(),
            &ExecuteMsg::RemoveTask { task_hash },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::Unauthorized {});

    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: Some(vec![CroncatQuery {
            contract_addr: "aloha".to_owned(),
            msg: Default::default(),
            check_result: true,
        }]),
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(40000, DENOM),
        )
        .unwrap();

    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();

    // Not a task owner tries to remove a task with queries
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("wrong_person"),
            tasks_addr.clone(),
            &ExecuteMsg::RemoveTask {
                task_hash: task_hash.clone(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::Unauthorized {});

    // No task
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("wrong_person"),
            tasks_addr.clone(),
            &ExecuteMsg::RemoveTask {
                task_hash: "wrong_task_hash".to_owned(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::NoTaskFound {});

    // Pause admin should be able to pause
    let res = app.execute_contract(
        Addr::unchecked(PAUSE_ADMIN),
        tasks_addr.clone(),
        &ExecuteMsg::PauseContract {},
        &[],
    );
    assert!(res.is_ok());

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr,
            &ExecuteMsg::RemoveTask { task_hash },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::ContractPaused {});
}

#[test]
fn is_valid_msg_negative_tests() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let _manager_addr = init_manager(&mut app, &factory_addr);
    let _agent_addr = init_agents(&mut app, &factory_addr);

    // no actions
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // no gas limit for wasm action
    let wasm_action = Action {
        msg: WasmMsg::Execute {
            contract_addr: "contract".to_owned(),
            msg: to_binary("wasm message").unwrap(),
            funds: vec![],
        }
        .into(),
        gas_limit: None,
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![wasm_action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NoGasLimit {});

    // Too many coins bank transfer
    let action = Action {
        msg: BankMsg::Send {
            to_address: "alice".to_owned(),
            amount: vec![coin(5, "coin1"), coin(2, "coin2"), coin(45, "coin3")],
        }
        .into(),
        gas_limit: None,
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // Zero coin bank transfer
    let action = Action {
        msg: BankMsg::Send {
            to_address: "alice".to_owned(),
            amount: vec![coin(0, "coin1")],
        }
        .into(),
        gas_limit: None,
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // Zero coins bank transfer
    let action = Action {
        msg: BankMsg::Send {
            to_address: "alice".to_owned(),
            amount: vec![],
        }
        .into(),
        gas_limit: None,
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // not supported message
    let action = Action {
        msg: BankMsg::Burn {
            amount: vec![coin(10, "coin1")],
        }
        .into(),
        gas_limit: None,
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // not supported message
    let action = Action {
        msg: StakingMsg::Delegate {
            validator: "alice".to_owned(),
            amount: coin(10, "coin1"),
        }
        .into(),
        gas_limit: None,
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // too many coins transfer inside wasm action
    let action = Action {
        msg: WasmMsg::Execute {
            contract_addr: "bestcontract".to_owned(),
            msg: Default::default(),
            funds: vec![coin(5, "coin1"), coin(2, "coin2"), coin(45, "coin3")],
        }
        .into(),
        gas_limit: Some(150_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // zero coins transfer inside wasm action
    let action = Action {
        msg: WasmMsg::Execute {
            contract_addr: "bestcontract".to_owned(),
            msg: Default::default(),
            funds: vec![coin(0, "coin1")],
        }
        .into(),
        gas_limit: Some(150_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // Zero cw20 transfer
    let action = Action {
        msg: WasmMsg::Execute {
            contract_addr: "bestcontract".to_owned(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "bob".to_owned(),
                amount: Uint128::new(0),
            })
            .unwrap(),
            funds: vec![],
        }
        .into(),
        gas_limit: Some(150_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // Zero cw20 send
    let action = Action {
        msg: WasmMsg::Execute {
            contract_addr: "best_contract".to_owned(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "bob".to_owned(),
                msg: Default::default(),
                amount: Uint128::new(0),
            })
            .unwrap(),
            funds: vec![],
        }
        .into(),
        gas_limit: Some(150_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // Multiple cw20 send
    let action1 = Action {
        msg: WasmMsg::Execute {
            contract_addr: "best_contract".to_owned(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "bob".to_owned(),
                msg: Default::default(),
                amount: Uint128::new(45),
            })
            .unwrap(),
            funds: vec![],
        }
        .into(),
        gas_limit: Some(150_000),
    };
    let action2 = Action {
        msg: WasmMsg::Execute {
            contract_addr: "best_contract2".to_owned(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: "bob".to_owned(),
                msg: Default::default(),
                amount: Uint128::new(45),
            })
            .unwrap(),
            funds: vec![],
        }
        .into(),
        gas_limit: Some(150_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action1, action2],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});

    // Multiple cw20 transfer
    let action1 = Action {
        msg: WasmMsg::Execute {
            contract_addr: "best_contract".to_owned(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "bob".to_owned(),
                amount: Uint128::new(45),
            })
            .unwrap(),
            funds: vec![],
        }
        .into(),
        gas_limit: Some(150_000),
    };
    let action2 = Action {
        msg: WasmMsg::Execute {
            contract_addr: "best_contract2".to_owned(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "bob".to_owned(),
                amount: Uint128::new(45),
            })
            .unwrap(),
            funds: vec![],
        }
        .into(),
        gas_limit: Some(150_000),
    };
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![action1, action2],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr,
            &ExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(30000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::InvalidAction {});
}

#[test]
fn query_slot_hashes_test() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let _ = init_manager(&mut app, &factory_addr);
    let _ = init_agents(&mut app, &factory_addr);

    // Test SlotHashes without tasks
    let hashes: SlotHashesResponse = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::SlotHashes { slot: None })
        .unwrap();
    assert_eq!(
        hashes,
        SlotHashesResponse {
            block_id: 0,
            block_task_hash: vec![],
            time_id: 0,
            time_task_hash: vec![]
        }
    );

    let hashes: SlotHashesResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::SlotHashes { slot: Some(12350) },
        )
        .unwrap();
    assert_eq!(
        hashes,
        SlotHashesResponse {
            block_id: 0,
            block_task_hash: vec![],
            time_id: 0,
            time_task_hash: vec![]
        }
    );

    let action = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT0).to_string(),
            amount: coins(10, DENOM),
        }
        .into(),
        gas_limit: Some(100_000),
    };
    let current_block: Uint64 = app.block_info().height.into(); // 12_345

    // Create several tasks
    let task1 = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some(current_block),
            end: None,
        })),
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task1),
            },
            &coins(30000, DENOM),
        )
        .unwrap();
    let block_task_hash1 = String::from_vec(res.data.unwrap().0).unwrap();

    let task2 = TaskRequest {
        interval: Interval::Immediate,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some(current_block),
            end: None,
        })),
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task2),
            },
            &coins(53000, DENOM),
        )
        .unwrap();
    let block_task_hash2 = String::from_vec(res.data.unwrap().0).unwrap();

    let task3 = TaskRequest {
        interval: Interval::Immediate,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some(current_block.saturating_add(5u64.into())),
            end: None,
        })),
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task3),
            },
            &coins(53000, DENOM),
        )
        .unwrap();
    let block_task_hash3 = String::from_vec(res.data.unwrap().0).unwrap();

    let task4 = TaskRequest {
        interval: Interval::Block(5),
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some(current_block),
            end: None,
        })),
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task4),
            },
            &coins(53000, DENOM),
        )
        .unwrap();
    let block_task_hash4 = String::from_vec(res.data.unwrap().0).unwrap();

    let task5 = TaskRequest {
        interval: Interval::Cron("* * * * * *".to_string()),
        boundary: None,
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task5),
            },
            &coins(53000, DENOM),
        )
        .unwrap();
    let time_task_hash1 = String::from_vec(res.data.unwrap().0).unwrap();

    let task6 = TaskRequest {
        interval: Interval::Cron("0 * * * * *".to_string()),
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &ExecuteMsg::CreateTask {
                task: Box::new(task6),
            },
            &coins(53000, DENOM),
        )
        .unwrap();
    let time_task_hash2 = String::from_vec(res.data.unwrap().0).unwrap();

    let hashes: SlotHashesResponse = app
        .wrap()
        .query_wasm_smart(tasks_addr.clone(), &QueryMsg::SlotHashes { slot: None })
        .unwrap();
    assert_eq!(
        hashes,
        SlotHashesResponse {
            block_id: current_block.u64() + 1,
            block_task_hash: vec![block_task_hash1, block_task_hash2],
            time_id: 1_571_797_420_000_000_000,
            time_task_hash: vec![time_task_hash1]
        }
    );

    let hashes: SlotHashesResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::SlotHashes {
                slot: Some(current_block.u64() + 5),
            },
        )
        .unwrap();
    assert_eq!(
        hashes,
        SlotHashesResponse {
            block_id: current_block.u64() + 5,
            block_task_hash: vec![block_task_hash3, block_task_hash4],
            time_id: 0,
            time_task_hash: vec![]
        }
    );

    // Current time is 1_571_797_419_879_305_533
    // Take the earliest timestamp with 00 seconds in it
    let hashes: SlotHashesResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &QueryMsg::SlotHashes {
                slot: Some(1_571_797_440_000_000_000),
            },
        )
        .unwrap();

    assert_eq!(
        hashes,
        SlotHashesResponse {
            block_id: 0,
            block_task_hash: vec![],
            time_id: 1_571_797_440_000_000_000,
            time_task_hash: vec![time_task_hash2]
        }
    );
}

#[test]
fn query_slot_tasks_total_test() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let _ = init_manager(&mut app, &factory_addr);
    let _ = init_agents(&mut app, &factory_addr);

    // Test SlotTasksTotal without tasks
    let slots: SlotTasksTotalResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::SlotTasksTotal { offset: None },
        )
        .unwrap();
    assert_eq!(
        slots,
        SlotTasksTotalResponse {
            block_tasks: 0,
            cron_tasks: 0,
            evented_tasks: 0
        }
    );

    let slots: SlotTasksTotalResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::SlotTasksTotal { offset: Some(5) },
        )
        .unwrap();
    assert_eq!(
        slots,
        SlotTasksTotalResponse {
            block_tasks: 0,
            cron_tasks: 0,
            evented_tasks: 0
        }
    );

    let action = Action {
        msg: BankMsg::Send {
            to_address: Addr::unchecked(PARTICIPANT0).to_string(),
            amount: coins(10, DENOM),
        }
        .into(),
        gas_limit: Some(100_000),
    };
    let current_block: Uint64 = app.block_info().height.into(); // 12_345

    // Create several tasks
    let task1 = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some(current_block),
            end: None,
        })),
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &ExecuteMsg::CreateTask {
            task: Box::new(task1),
        },
        &coins(30000, DENOM),
    )
    .unwrap();

    let task2 = TaskRequest {
        interval: Interval::Immediate,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some(current_block),
            end: None,
        })),
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &ExecuteMsg::CreateTask {
            task: Box::new(task2),
        },
        &coins(53000, DENOM),
    )
    .unwrap();

    let task3 = TaskRequest {
        interval: Interval::Immediate,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some(current_block.saturating_add(5u64.into())),
            end: None,
        })),
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &ExecuteMsg::CreateTask {
            task: Box::new(task3),
        },
        &coins(53000, DENOM),
    )
    .unwrap();

    let task4 = TaskRequest {
        interval: Interval::Block(5),
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: Some(current_block),
            end: None,
        })),
        stop_on_fail: false,
        actions: vec![action.clone()],
        queries: None,
        transforms: None,
        cw20: None,
    };
    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &ExecuteMsg::CreateTask {
            task: Box::new(task4),
        },
        &coins(53000, DENOM),
    )
    .unwrap();

    // Cron task
    // Scheduled for 1_571_797_420_000_000_000
    let task5 = TaskRequest {
        interval: Interval::Cron("* * * * * *".to_string()),
        boundary: None,
        stop_on_fail: false,
        actions: vec![action],
        queries: None,
        transforms: None,
        cw20: None,
    };
    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &ExecuteMsg::CreateTask {
            task: Box::new(task5),
        },
        &coins(53000, DENOM),
    )
    .unwrap();

    // Takes block 12345 and timestamp 1571797410000000000
    // No task scheduled for these slots
    let slots: SlotTasksTotalResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::SlotTasksTotal { offset: None },
        )
        .unwrap();
    assert_eq!(
        slots,
        SlotTasksTotalResponse {
            block_tasks: 0,
            cron_tasks: 0,
            evented_tasks: 0
        }
    );

    // Takes block 12346 and timestamp 1571797420000000000
    // Tasks 1, 2 (block) and 5(time) are scheduled
    let slots: SlotTasksTotalResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::SlotTasksTotal { offset: Some(1) },
        )
        .unwrap();
    assert_eq!(
        slots,
        SlotTasksTotalResponse {
            block_tasks: 2,
            cron_tasks: 1,
            evented_tasks: 0
        }
    );

    // Takes block 12350 and timestamp 1571797460000000000
    // Tasks 3 and 4 (block) are scheduled
    let slots: SlotTasksTotalResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &QueryMsg::SlotTasksTotal { offset: Some(5) },
        )
        .unwrap();
    assert_eq!(
        slots,
        SlotTasksTotalResponse {
            block_tasks: 2,
            cron_tasks: 0,
            evented_tasks: 0
        }
    );

    // Add 5 blocks
    app.update_block(add_little_time);
    app.update_block(add_little_time);
    app.update_block(add_little_time);
    app.update_block(add_little_time);
    app.update_block(add_little_time);

    let slots: SlotTasksTotalResponse = app
        .wrap()
        .query_wasm_smart(tasks_addr, &QueryMsg::SlotTasksTotal { offset: None })
        .unwrap();
    assert_eq!(
        slots,
        SlotTasksTotalResponse {
            block_tasks: 4,
            cron_tasks: 1,
            evented_tasks: 0
        }
    );
}

#[test]
fn poc_case1_case2() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);
    let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let manager_addr = init_manager(&mut app, &factory_addr);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &instantiate_msg, &factory_addr);
    let task2: Addr = tasks_addr.clone();

    const DEFAULT_FEE: u64 = 5;

    activate_agent(&mut app, &agents_addr);

    //init agent 2
    app.execute_contract(
        Addr::unchecked(AGENT1),
        agents_addr.clone(),
        &croncat_agents::msg::ExecuteMsg::RegisterAgent {
            payable_account_id: None,
        },
        &[],
    )
    .unwrap();

    // Check in - will error but force for later
    app.execute_contract(
        Addr::unchecked(AGENT1),
        agents_addr.clone(),
        &croncat_agents::msg::ExecuteMsg::CheckInAgent {},
        &[],
    )
    .unwrap_err();

    // wait 100 blocks - simply added this function to /helpers.rs to add 100
    // blocks and 1900 seconds
    app.update_block(|block| add_seconds_to_block(block, 3600));
    app.update_block(|block| increment_block_height(block, Some(100)));

    // Add random task #1
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: "bob".to_owned(),
                amount: coins(45, DENOM),
            }
            .into(),
            gas_limit: None,
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let _res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap();
    // Add random task #2
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: "bob1".to_owned(),
                amount: coins(45, DENOM),
            }
            .into(),
            gas_limit: None,
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let _res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap();
    // Add random task #3
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: "bob2".to_owned(),
                amount: coins(45, DENOM),
            }
            .into(),
            gas_limit: None,
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let _res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap();
    // random task 4
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: "bob3".to_owned(),
                amount: coins(45, DENOM),
            }
            .into(),
            gas_limit: None,
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let _res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap();
    // Add TARGET BLOCK TASK 1
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Block(3),
        // repeat it three times
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: None,
            end: Some((app.block_info().height + 8).into()),
        })),
        stop_on_fail: false,
        actions: vec![
            Action {
                msg: BankMsg::Send {
                    to_address: "alice".to_owned(),
                    amount: coins(123, DENOM),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: BankMsg::Send {
                    to_address: "bob".to_owned(),
                    amount: coins(321, DENOM),
                }
                .into(),
                gas_limit: None,
            },
        ],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap();
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();
    let _t2: String = task_hash.clone();
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksQueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();
    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let _expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees + 321.0 + 123.0
    } as u128;

    // Add more time to pass agent nomination
    app.update_block(|block| add_seconds_to_block(block, 3600));
    app.update_block(|block| increment_block_height(block, Some(100)));

    // Check in - this will pass
    app.execute_contract(
        Addr::unchecked(AGENT1),
        agents_addr.clone(),
        &croncat_agents::msg::ExecuteMsg::CheckInAgent {},
        &[],
    )
    .unwrap();

    // ###### END SETUP ###

    let current_task: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksQueryMsg::CurrentTask {},
        )
        .unwrap();

    // random task 4 from above - only
    let _current_hash: String = current_task.task.unwrap().task_hash;

    // Agent 0 legitimately calls the tasks contract to execute a task
    let _res0 = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr.clone(),
            &ManagerExecuteMsg::ProxyCall { task_hash: None },
            &[],
        )
        .unwrap();

    // ### CASE 1 ###
    // Prove no evented tasks
    let tasks_for_agent: Option<Vec<croncat_sdk_tasks::types::TaskInfo>> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_sdk_tasks::msg::TasksQueryMsg::EventedTasks {
                start: None,
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(tasks_for_agent.unwrap().is_empty());

    // Agent 1 is able to call a block task directly by its hash
    // TARGET BLOCK TASK 1 from above
    // this is not evented so it should not be able to be called directly by any agent
    let res1_error: croncat_manager::ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT1),
            manager_addr.clone(),
            &ManagerExecuteMsg::ProxyCall {
                task_hash: Some(task_hash),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        res1_error,
        croncat_manager::ContractError::NoTaskForAgent {}
    );

    // ### CASE 2 ##
    // Add TARGET BLOCK TASK 1
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Block(3),
        // repeat it three times
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: None,
            end: Some((app.block_info().height + 8).into()),
        })),
        stop_on_fail: false,
        actions: vec![
            Action {
                msg: BankMsg::Send {
                    to_address: "alice".to_owned(),
                    amount: coins(123, DENOM),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: BankMsg::Send {
                    to_address: "bob".to_owned(),
                    amount: coins(321, DENOM),
                }
                .into(),
                gas_limit: None,
            },
        ],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            task2.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap();
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();
    let _t2: String = task_hash.clone();
    let _task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            task2.clone(),
            &croncat_sdk_tasks::msg::TasksQueryMsg::Task { task_hash },
        )
        .unwrap();

    // create a task with the owner address - should error
    // Actions message unsupported or invalid message data
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: WasmMsg::Execute {
                contract_addr: manager_addr.to_string(),
                msg: to_binary(&croncat_sdk_manager::msg::ManagerExecuteMsg::OwnerWithdraw {})
                    .unwrap(),
                funds: Default::default(),
            }
            .into(),
            gas_limit: Some(250_000),
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };

    // passing message with uppercase manager address
    let _mod_balances = init_mod_balances(&mut app, &factory_addr);
    let _res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            task2.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap_err();
    let malicious_task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: WasmMsg::Execute {
                contract_addr: manager_addr.to_string().to_uppercase(),
                msg: to_binary(&croncat_sdk_manager::msg::ManagerExecuteMsg::OwnerWithdraw {})
                    .unwrap(),
                funds: Default::default(),
            }
            .into(),
            gas_limit: Some(250_000),
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };

    // Need this to fail to check correct coverage
    let error: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            task2.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(malicious_task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        error,
        ContractError::Std(StdError::generic_err(
            "Invalid input: address not normalized"
        ))
    );

    let malicious_task2 = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: manager_addr.to_string().to_uppercase(),
                // to_address: "bob".to_owned(),
                amount: coins(123, DENOM),
            }
            .into(),
            gas_limit: Some(250_000),
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };

    // Need this to fail to check correct coverage
    let error: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            task2,
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(malicious_task2),
            },
            &coins(600_000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(error, ContractError::InvalidAddress {});
}

#[test]
fn invalid_gas_config() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);
    let code_id = app.store_code(contracts::croncat_tasks_contract());

    let mut instantiate_msg: InstantiateMsg = default_instantiate_msg();
    // We introduce an invalid value to the Task instantiate message
    instantiate_msg.slot_granularity_time = Some(0);

    // Set up errors we'll check against
    let err_slot_granularity_time = ContractError::InvalidZeroValue {
        field: "slot_granularity_time".to_string(),
    };
    let err_invalid_gas = ContractError::InvalidGas {};

    let module_instantiate_info = ModuleInstantiateInfo {
        code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&instantiate_msg).unwrap(),
        contract_name: "tasks".to_owned(),
    };
    let mut err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            factory_addr.to_owned(),
            &croncat_factory::msg::ExecuteMsg::Deploy {
                kind: VersionKind::Tasks,
                module_instantiate_info,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, err_slot_granularity_time);

    // Reset the slot_granularity_time and check an invalid value for gas_limit
    instantiate_msg.slot_granularity_time = None;
    // This value is too low
    instantiate_msg.gas_limit = Some(1);

    let module_instantiate_info = ModuleInstantiateInfo {
        code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&instantiate_msg).unwrap(),
        contract_name: "tasks".to_owned(),
    };
    err = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            factory_addr.to_owned(),
            &croncat_factory::msg::ExecuteMsg::Deploy {
                kind: VersionKind::Tasks,
                module_instantiate_info,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, err_invalid_gas);

    // Now we allow for successful instantiation by setting it back to a working instantiate message
    instantiate_msg.gas_limit = None;

    let module_instantiate_info = ModuleInstantiateInfo {
        code_id,
        version: [0, 1],
        commit_id: "commit1".to_owned(),
        checksum: "checksum2".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&instantiate_msg).unwrap(),
        contract_name: "tasks".to_owned(),
    };
    assert!(
        app.execute_contract(
            Addr::unchecked(ADMIN),
            factory_addr.to_owned(),
            &croncat_factory::msg::ExecuteMsg::Deploy {
                kind: VersionKind::Tasks,
                module_instantiate_info,
            },
            &[],
        )
        .is_ok(),
        "Tasks contract should instantiate successfully"
    );

    // Get the Tasks contract address
    let metadata: ContractMetadataResponse = app
        .wrap()
        .query_wasm_smart(
            &factory_addr,
            &croncat_factory::msg::QueryMsg::LatestContract {
                contract_name: "tasks".to_owned(),
            },
        )
        .unwrap();
    let tasks_addr = metadata.metadata.unwrap().contract_addr;

    // Ensure that invalid config updates fail
    // Test failure when gas_limit is too low (should be more than the sum of other gas_* values)
    let mut update_msg = UpdateConfigMsg {
        croncat_factory_addr: Some("fixed_croncat_factory_addr".to_owned()),
        croncat_manager_key: Some(("manager2".to_owned(), [2, 2])),
        croncat_agents_key: Some(("agents2".to_owned(), [2, 2])),
        slot_granularity_time: Some(54),
        gas_base_fee: Some(1),
        gas_action_fee: Some(2),
        gas_query_fee: Some(3),
        // This should be higher and will fail
        gas_limit: Some(4),
    };

    let mut msg = WasmMsg::Execute {
        contract_addr: tasks_addr.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateConfig(update_msg.clone())).unwrap(),
        funds: vec![],
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            factory_addr.clone(),
            &FactoryExecuteMsg::Proxy { msg },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, err_invalid_gas);

    // Reset gas_limit and set invalid value for slot_granularity_time
    update_msg.gas_limit = None;
    update_msg.slot_granularity_time = Some(0);

    msg = WasmMsg::Execute {
        contract_addr: tasks_addr.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateConfig(update_msg)).unwrap(),
        funds: vec![],
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            factory_addr,
            &FactoryExecuteMsg::Proxy { msg },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, err_slot_granularity_time);
}

/// Check for instantiate pause admin scenarios of pass/fail
/// Check for pause & unpause scenarios of pass/fail
#[test]
fn pause_admin_cases() {
    let mut app = default_app();

    let factory_code_id = app.store_code(contracts::croncat_factory_contract());
    let tasks_code_id = app.store_code(contracts::croncat_tasks_contract());

    let init_msg = croncat_sdk_factory::msg::FactoryInstantiateMsg {
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

    let init_tasks_contract_msg = InstantiateMsg {
        chain_name: "cron".to_owned(),
        version: Some("0.1".to_owned()),
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        croncat_manager_key: ("definitely_not_manager".to_owned(), [4, 2]),
        croncat_agents_key: ("definitely_not_agents".to_owned(), [42, 0]),
        slot_granularity_time: Some(10),
        gas_base_fee: Some(1),
        gas_action_fee: Some(2),
        gas_query_fee: Some(3),
        gas_limit: Some(10),
    };
    // Attempt to initialize with short address for pause_admin
    let mut init_tasks_contract_msg_short_addr = init_tasks_contract_msg.clone();
    init_tasks_contract_msg_short_addr.pause_admin = Addr::unchecked(ANYONE);
    // Attempt to initialize with same owner address for pause_admin
    let mut init_tasks_contract_msg_same_owner = init_tasks_contract_msg.clone();
    init_tasks_contract_msg_same_owner.pause_admin = Addr::unchecked(ADMIN);

    // Should fail: shorty addr
    let tasks_module_instantiate_info = croncat_sdk_factory::msg::ModuleInstantiateInfo {
        code_id: tasks_code_id,
        version: [0, 1],
        commit_id: "some".to_owned(),
        checksum: "qwe123".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&init_tasks_contract_msg_short_addr).unwrap(),
        contract_name: "tasks".to_owned(),
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            croncat_factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryExecuteMsg::Deploy {
                kind: croncat_sdk_factory::msg::VersionKind::Tasks,
                module_instantiate_info: tasks_module_instantiate_info,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidPauseAdmin {});

    // Should fail: same as owner
    let tasks_module_instantiate_info = croncat_sdk_factory::msg::ModuleInstantiateInfo {
        code_id: tasks_code_id,
        version: [0, 1],
        commit_id: "some".to_owned(),
        checksum: "qwe123".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&init_tasks_contract_msg_same_owner).unwrap(),
        contract_name: "tasks".to_owned(),
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            croncat_factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryExecuteMsg::Deploy {
                kind: croncat_sdk_factory::msg::VersionKind::Tasks,
                module_instantiate_info: tasks_module_instantiate_info,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidPauseAdmin {});

    // Now, we do a working furr shurr case
    let tasks_module_instantiate_info = croncat_sdk_factory::msg::ModuleInstantiateInfo {
        code_id: tasks_code_id,
        version: [0, 1],
        commit_id: "some".to_owned(),
        checksum: "qwe123".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&init_tasks_contract_msg).unwrap(),
        contract_name: "tasks".to_owned(),
    };

    // Successfully deploy agents contract
    app.execute_contract(
        Addr::unchecked(ADMIN),
        croncat_factory_addr.clone(),
        &croncat_sdk_factory::msg::FactoryExecuteMsg::Deploy {
            kind: croncat_sdk_factory::msg::VersionKind::Tasks,
            module_instantiate_info: tasks_module_instantiate_info,
        },
        &[],
    )
    .unwrap();

    // Get tasks contract address
    let tasks_contracts: ContractMetadataResponse = app
        .wrap()
        .query_wasm_smart(
            croncat_factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryQueryMsg::LatestContract {
                contract_name: "tasks".to_string(),
            },
        )
        .unwrap();
    assert!(
        tasks_contracts.metadata.is_some(),
        "Should be contract metadata"
    );
    let tasks_metadata = tasks_contracts.metadata.unwrap();
    let croncat_tasks_addr = tasks_metadata.contract_addr;

    // Owner Should not be able to pause, not pause_admin
    let error: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            croncat_tasks_addr.clone(),
            &ExecuteMsg::PauseContract {},
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(error, ContractError::Unauthorized {});
    // Anyone Should not be able to pause, not pause_admin
    let error: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            croncat_tasks_addr.clone(),
            &ExecuteMsg::PauseContract {},
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(error, ContractError::Unauthorized {});

    // Pause admin should be able to pause
    let res = app.execute_contract(
        Addr::unchecked(PAUSE_ADMIN),
        croncat_tasks_addr.clone(),
        &ExecuteMsg::PauseContract {},
        &[],
    );
    assert!(res.is_ok());

    // Check the pause query is valid
    let is_paused: bool = app
        .wrap()
        .query_wasm_smart(croncat_tasks_addr.clone(), &QueryMsg::Paused {})
        .unwrap();
    assert!(is_paused);

    // Pause Admin Should not be able to unpause
    let error: ContractError = app
        .execute_contract(
            Addr::unchecked(PAUSE_ADMIN),
            croncat_tasks_addr.clone(),
            &ExecuteMsg::UnpauseContract {},
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(error, ContractError::Unauthorized {});
    // Anyone Should not be able to unpause
    let error: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            croncat_tasks_addr.clone(),
            &ExecuteMsg::UnpauseContract {},
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(error, ContractError::Unauthorized {});

    // Owner should be able to unpause
    let res = app.execute_contract(
        Addr::unchecked(ADMIN),
        croncat_factory_addr,
        &croncat_sdk_factory::msg::FactoryExecuteMsg::Proxy {
            msg: WasmMsg::Execute {
                contract_addr: croncat_tasks_addr.to_string(),
                msg: to_binary(&ExecuteMsg::UnpauseContract {}).unwrap(),
                funds: vec![],
            },
        },
        &[],
    );
    assert!(res.is_ok());

    // Confirm unpaused
    let is_paused: bool = app
        .wrap()
        .query_wasm_smart(croncat_tasks_addr, &QueryMsg::Paused {})
        .unwrap();
    assert!(!is_paused);
}
