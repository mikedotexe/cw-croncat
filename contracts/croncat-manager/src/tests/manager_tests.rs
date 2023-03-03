use cosmwasm_std::Attribute;
use cosmwasm_std::BlockInfo;
use cosmwasm_std::{coins, to_binary, Addr, BankMsg, Binary, Coin, Uint128, WasmMsg};
use croncat_mod_balances::types::HasBalanceComparator;
use croncat_sdk_agents::msg::ExecuteMsg::RegisterAgent;
use croncat_sdk_core::internal_messages::agents::AgentWithdrawOnRemovalArgs;
use croncat_sdk_factory::msg::ContractMetadataResponse;
use croncat_sdk_manager::{
    msg::AgentWithdrawCallback,
    types::{Config, TaskBalance, TaskBalanceResponse, UpdateConfig},
};
use croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask;
use croncat_sdk_tasks::types::TaskRequest;
use croncat_sdk_tasks::types::{
    Action, Boundary, BoundaryHeight, BoundaryTime, CroncatQuery, Interval, TaskResponse, Transform,
};
use cw20::{Cw20Coin, Cw20CoinVerified, Cw20ExecuteMsg, Cw20QueryMsg};
use cw_storage_plus::KeyDeserialize;

use crate::tests::PARTICIPANT3;
use crate::tests::PAUSE_ADMIN;
use crate::{
    contract::DEFAULT_FEE,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ReceiveMsg},
    tests::{
        helpers::{
            default_app, default_instantiate_message, init_manager, init_mod_balances,
            query_manager_config, support_new_cw20,
        },
        helpers::{init_factory, query_manager_balances},
        ADMIN, AGENT1, AGENT2, ANYONE, DENOM, PARTICIPANT2,
    },
    ContractError,
};
use cosmwasm_std::{coin, StdError};
use croncat_sdk_core::types::GasPrice;
use croncat_sdk_manager::msg::ManagerExecuteMsg::ProxyCall;
use cw_boolean_contract::msgs::execute_msg::ExecuteMsg::Toggle;
use cw_multi_test::{BankSudo, Executor};

use super::{
    contracts,
    helpers::{init_agents, init_boolean, init_tasks},
    PARTICIPANT0, PARTICIPANT1,
};
use super::{
    helpers::{activate_agent, add_little_time, init_cw20, query_users_manager},
    AGENT0,
};

mod instantiate_tests {
    use crate::tests::{PARTICIPANT3, PAUSE_ADMIN};
    use croncat_sdk_factory::msg::{ModuleInstantiateInfo, VersionKind};

    use super::*;

    #[test]
    fn default_init() {
        let mut app = default_app();
        let instantiate_msg: InstantiateMsg = default_instantiate_message();
        let factory_addr = init_factory(&mut app);
        let send_funds: &[Coin] = &[coin(600, DENOM)];

        let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, send_funds);
        let config = query_manager_config(&app, &manager_addr);

        let expected_config = Config {
            owner_addr: factory_addr.clone(),
            pause_admin: Addr::unchecked(PAUSE_ADMIN),
            croncat_factory_addr: factory_addr,
            croncat_tasks_key: ("tasks".to_owned(), [0, 1]),
            croncat_agents_key: ("agents".to_owned(), [0, 1]),
            agent_fee: DEFAULT_FEE,
            treasury_fee: DEFAULT_FEE,
            gas_price: Default::default(),
            cw20_whitelist: vec![],
            native_denom: DENOM.to_owned(),
            limit: 100,
            treasury_addr: None,
        };
        assert_eq!(config, expected_config);
    }

    #[test]
    fn custom_init() {
        let mut app = default_app();
        let factory_addr = init_factory(&mut app);

        let instantiate_msg: InstantiateMsg = InstantiateMsg {
            version: Some("0.1".to_owned()),
            croncat_tasks_key: (AGENT1.to_owned(), [0, 1]),
            croncat_agents_key: (AGENT2.to_owned(), [0, 1]),
            pause_admin: Addr::unchecked(PAUSE_ADMIN),
            gas_price: Some(GasPrice {
                numerator: 10,
                denominator: 20,
                gas_adjustment_numerator: 30,
            }),
            treasury_addr: Some(AGENT2.to_owned()),
            cw20_whitelist: Some(vec![PARTICIPANT3.to_owned()]),
        };
        let attach_funds = vec![coin(5000, "denom"), coin(2400, DENOM)];

        app.sudo(
            BankSudo::Mint {
                to_address: ADMIN.to_owned(),
                amount: attach_funds.clone(),
            }
            .into(),
        )
        .unwrap();

        // Test attaching extra tokens FAILs
        let code_id = app.store_code(contracts::croncat_manager_contract());
        let module_instantiate_info = ModuleInstantiateInfo {
            code_id,
            version: [0, 1],
            commit_id: "commit1".to_owned(),
            checksum: "checksum2".to_owned(),
            changelog_url: None,
            schema: None,
            msg: to_binary(&instantiate_msg).unwrap(),
            contract_name: "manager".to_owned(),
        };
        let error: ContractError = app
            .execute_contract(
                Addr::unchecked(ADMIN),
                factory_addr.to_owned(),
                &croncat_factory::msg::ExecuteMsg::Deploy {
                    kind: VersionKind::Manager,
                    module_instantiate_info,
                },
                &attach_funds,
            )
            .unwrap_err()
            .downcast()
            .unwrap();
        assert_eq!(error, ContractError::RedundantFunds {});

        let attach_funds = coins(2400, DENOM);

        let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &attach_funds);

        let config = query_manager_config(&app, &manager_addr);

        let expected_config = Config {
            pause_admin: Addr::unchecked(PAUSE_ADMIN),
            owner_addr: factory_addr.clone(),
            croncat_factory_addr: factory_addr,
            croncat_tasks_key: (AGENT1.to_owned(), [0, 1]),
            croncat_agents_key: (AGENT2.to_owned(), [0, 1]),
            agent_fee: DEFAULT_FEE,
            treasury_fee: DEFAULT_FEE,
            gas_price: GasPrice {
                numerator: 10,
                denominator: 20,
                gas_adjustment_numerator: 30,
            },
            cw20_whitelist: vec![Addr::unchecked(PARTICIPANT3)],
            native_denom: DENOM.to_string(),
            limit: 100,
            treasury_addr: Some(Addr::unchecked(AGENT2)),
        };
        assert_eq!(config, expected_config);

        let manager_balances = query_manager_balances(&app, &manager_addr);
        assert_eq!(manager_balances, Uint128::new(2400));
    }

    #[test]
    fn invalid_inits() {
        let mut app = default_app();
        let code_id = app.store_code(contracts::croncat_manager_contract());
        // Invalid gas price
        let instantiate_msg: InstantiateMsg = InstantiateMsg {
            gas_price: Some(GasPrice {
                numerator: 0,
                denominator: 1,
                gas_adjustment_numerator: 2,
            }),
            ..default_instantiate_message()
        };

        let error: ContractError = app
            .instantiate_contract(
                code_id,
                Addr::unchecked(ADMIN),
                &instantiate_msg,
                &[],
                "croncat-manager",
                None,
            )
            .unwrap_err()
            .downcast()
            .unwrap();
        assert_eq!(error, ContractError::InvalidGasPrice {});

        // Bad owner_addr
        let instantiate_msg: InstantiateMsg = InstantiateMsg {
            pause_admin: Addr::unchecked("BAD_INPUT"),
            ..default_instantiate_message()
        };

        let error: ContractError = app
            .instantiate_contract(
                code_id,
                Addr::unchecked(ADMIN),
                &instantiate_msg,
                &[],
                "croncat-manager",
                None,
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
    }
}

#[test]
fn update_config() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();

    let attach_funds = vec![coin(5000, "denom"), coin(2400, DENOM)];
    app.sudo(
        BankSudo::Mint {
            to_address: ADMIN.to_owned(),
            amount: attach_funds,
        }
        .into(),
    )
    .unwrap();

    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);

    let update_cfg_msg = UpdateConfig {
        agent_fee: Some(0),
        treasury_fee: Some(0),
        gas_price: Some(GasPrice {
            numerator: 555,
            denominator: 666,
            gas_adjustment_numerator: 777,
        }),
        croncat_tasks_key: Some(("new_key_tasks".to_owned(), [0, 1])),
        croncat_agents_key: Some(("new_key_agents".to_owned(), [0, 1])),
        treasury_addr: Some(ANYONE.to_owned()),
        cw20_whitelist: Some(vec!["randomcw20".to_owned()]),
    };

    app.execute_contract(
        Addr::unchecked(ADMIN),
        factory_addr.clone(),
        &croncat_sdk_factory::msg::FactoryExecuteMsg::Proxy {
            msg: WasmMsg::Execute {
                contract_addr: manager_addr.to_string(),
                msg: to_binary(&ExecuteMsg::UpdateConfig(Box::new(update_cfg_msg))).unwrap(),
                funds: vec![],
            },
        },
        &[],
    )
    .unwrap();
    let config = query_manager_config(&app, &manager_addr);
    let expected_config = Config {
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        owner_addr: factory_addr.clone(),
        croncat_factory_addr: factory_addr,
        croncat_tasks_key: ("new_key_tasks".to_owned(), [0, 1]),
        croncat_agents_key: ("new_key_agents".to_owned(), [0, 1]),
        agent_fee: 0,
        treasury_fee: 0,
        gas_price: GasPrice {
            numerator: 555,
            denominator: 666,
            gas_adjustment_numerator: 777,
        },
        cw20_whitelist: vec![Addr::unchecked("randomcw20")],
        native_denom: DENOM.to_owned(),
        limit: 100,
        treasury_addr: Some(Addr::unchecked(ANYONE)),
    };
    assert_eq!(config, expected_config);
}

#[test]
fn invalid_updates_config() {
    let mut app = default_app();
    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let factory_addr = init_factory(&mut app);

    let attach_funds = vec![coin(5000, "denom"), coin(2400, DENOM)];
    app.sudo(
        BankSudo::Mint {
            to_address: ADMIN.to_owned(),
            amount: attach_funds,
        }
        .into(),
    )
    .unwrap();

    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);

    // Unauthorized
    let update_cfg_msg = UpdateConfig {
        agent_fee: Some(0),
        treasury_fee: Some(2),
        gas_price: Some(GasPrice {
            numerator: 555,
            denominator: 666,
            gas_adjustment_numerator: 777,
        }),
        croncat_tasks_key: Some(("new_key_tasks".to_owned(), [0, 1])),
        croncat_agents_key: Some(("new_key_agents".to_owned(), [0, 1])),
        treasury_addr: Some(ANYONE.to_owned()),
        cw20_whitelist: Some(vec!["randomcw20".to_owned()]),
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            manager_addr.clone(),
            &ExecuteMsg::UpdateConfig(Box::new(update_cfg_msg)),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    // Invalid gas_price
    let update_cfg_msg = UpdateConfig {
        agent_fee: Some(0),
        treasury_fee: Some(2),
        gas_price: Some(GasPrice {
            numerator: 555,
            denominator: 0,
            gas_adjustment_numerator: 777,
        }),
        croncat_tasks_key: Some(("new_key_tasks".to_owned(), [0, 1])),
        croncat_agents_key: Some(("new_key_agents".to_owned(), [0, 1])),
        treasury_addr: Some(ANYONE.to_owned()),
        cw20_whitelist: Some(vec!["randomcw20".to_owned()]),
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryExecuteMsg::Proxy {
                msg: WasmMsg::Execute {
                    contract_addr: manager_addr.to_string(),
                    msg: to_binary(&ExecuteMsg::UpdateConfig(Box::new(update_cfg_msg))).unwrap(),
                    funds: vec![],
                },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidGasPrice {});
}

#[test]
fn cw20_receive() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);

    let cw20_addr = init_cw20(&mut app);
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            cw20_addr.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: manager_addr.to_string(),
                amount: Uint128::new(555),
                msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NotSupportedCw20 {});

    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        cw20_addr.as_str(),
    );
    app.execute_contract(
        Addr::unchecked(ADMIN),
        cw20_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: manager_addr.to_string(),
            amount: Uint128::new(555),
            msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    let wallet_balances = query_users_manager(&app, &manager_addr, ADMIN);
    assert_eq!(
        wallet_balances,
        vec![Cw20CoinVerified {
            address: cw20_addr,
            amount: Uint128::new(555),
        }]
    );
}

#[test]
fn cw20_bad_messages() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();

    let send_funds: &[Coin] = &[coin(600, DENOM)];
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, send_funds);

    let cw20_addr = init_cw20(&mut app);
    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        cw20_addr.as_str(),
    );
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            cw20_addr.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: manager_addr.to_string(),
                amount: Uint128::new(555),
                msg: Default::default(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(
        err,
        ContractError::Std(StdError::parse_err(
            "croncat_sdk_manager::msg::ManagerReceiveMsg",
            "EOF while parsing a JSON value."
        ))
    );

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            cw20_addr,
            &cw20::Cw20ExecuteMsg::Send {
                contract: manager_addr.to_string(),
                amount: Uint128::new(555),
                msg: to_binary(&true).unwrap(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(
        err,
        ContractError::Std(StdError::parse_err(
            "croncat_sdk_manager::msg::ManagerReceiveMsg",
            "Expected to parse either a `true`, `false`, or a `null`."
        ))
    );
}

#[test]
fn users_withdraws() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();

    let send_funds: &[Coin] = &[coin(600, DENOM)];
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, send_funds);

    // refill balances
    let cw20_addr = init_cw20(&mut app);
    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        cw20_addr.as_str(),
    );
    app.execute_contract(
        Addr::unchecked(ADMIN),
        cw20_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: manager_addr.to_string(),
            amount: Uint128::new(1000),
            msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Withdraw half
    let user_cw20_balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: ADMIN.to_owned(),
            },
        )
        .unwrap();

    app.execute_contract(
        Addr::unchecked(ADMIN),
        manager_addr.clone(),
        &ExecuteMsg::UserWithdraw { limit: None },
        &[],
    )
    .unwrap();

    // Check it got withdrawn

    // Check it updated on manager
    let manager_wallet_balance = query_users_manager(&app, &manager_addr, ADMIN);
    assert_eq!(manager_wallet_balance, vec![]);

    let fully_withdrawn_user_balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr,
            &cw20::Cw20QueryMsg::Balance {
                address: ADMIN.to_owned(),
            },
        )
        .unwrap();
    assert_eq!(
        fully_withdrawn_user_balance.balance,
        user_cw20_balance.balance + Uint128::new(1000)
    );
}

#[test]
fn failed_users_withdraws() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();

    let send_funds: &[Coin] = &[coin(600, DENOM)];
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, send_funds);

    let cw20_addr = init_cw20(&mut app);
    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        cw20_addr.as_str(),
    );

    // try to withdraw empty balances
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            manager_addr.clone(),
            &ExecuteMsg::UserWithdraw { limit: None },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::EmptyBalance {});

    // refill balances
    app.execute_contract(
        Addr::unchecked(ADMIN),
        cw20_addr,
        &cw20::Cw20ExecuteMsg::Send {
            contract: manager_addr.to_string(),
            amount: Uint128::new(1000),
            msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Another user tries to withdraw
    // No steals here
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            manager_addr,
            &ExecuteMsg::UserWithdraw { limit: None },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::EmptyBalance {});
}

#[test]
fn withdraw_balances() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();

    let attach_funds = vec![coin(2400, DENOM)];
    app.sudo(
        BankSudo::Mint {
            to_address: ADMIN.to_owned(),
            amount: attach_funds.clone(),
        }
        .into(),
    )
    .unwrap();

    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &attach_funds);

    // refill balance
    let cw20_addr = init_cw20(&mut app);
    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        cw20_addr.as_str(),
    );

    app.execute_contract(
        Addr::unchecked(ADMIN),
        cw20_addr,
        &cw20::Cw20ExecuteMsg::Send {
            contract: manager_addr.to_string(),
            amount: Uint128::new(1000),
            msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    //Check if sending Cw20 does not effect on treasury
    let treasury_balance = query_manager_balances(&app, &manager_addr);

    assert_eq!(treasury_balance, attach_funds[0].amount);

    // Withdraw all of balances
    app.execute_contract(
        factory_addr.clone(),
        manager_addr.clone(),
        &ExecuteMsg::OwnerWithdraw {},
        &[],
    )
    .unwrap();

    // Can't withdraw empty
    let err: ContractError = app
        .execute_contract(
            factory_addr.clone(),
            manager_addr,
            &ExecuteMsg::OwnerWithdraw {},
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::EmptyBalance {});
}

#[test]
fn failed_move_balances() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);

    let attach_funds = vec![coin(2400, DENOM), coin(5000, "denom")];
    app.sudo(
        BankSudo::Mint {
            to_address: ADMIN.to_owned(),
            amount: attach_funds,
        }
        .into(),
    )
    .unwrap();

    // refill balance
    let cw20_addr = init_cw20(&mut app);
    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        cw20_addr.as_str(),
    );
    app.execute_contract(
        Addr::unchecked(ADMIN),
        cw20_addr,
        &cw20::Cw20ExecuteMsg::Send {
            contract: manager_addr.to_string(),
            amount: Uint128::new(1000),
            msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Withdraw not by owner
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            manager_addr,
            &ExecuteMsg::OwnerWithdraw {},
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn simple_bank_transfers_block() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    activate_agent(&mut app, &agents_addr);

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
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees + 45.0
    } as u128;

    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();
    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // action done
    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, coins(45, DENOM));

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        600_000 - expected_gone_amount,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr.clone(), &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128));

    // Checking we don't get same task over and over
    // Check multi-action transfer

    // withdraw rewards so it's clear before second test
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::AgentWithdraw(None),
        &[],
    )
    .unwrap();
    app.execute_contract(
        factory_addr.clone(),
        manager_addr.clone(),
        &ExecuteMsg::OwnerWithdraw {},
        &[],
    )
    .unwrap();

    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![
            Action {
                msg: BankMsg::Send {
                    to_address: "bob2".to_owned(),
                    amount: coins(45, DENOM),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: BankMsg::Send {
                    to_address: "alice".to_owned(),
                    amount: coins(125, DENOM),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: BankMsg::Send {
                    to_address: "lucy".to_owned(),
                    amount: coins(333, DENOM),
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
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees + 45.0 + 125.0 + 333.0
    } as u128;

    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();

    // crate::tests::helpers::check_task_chain(&app, &tasks_addr, &agents_addr);

    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();
    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // action done
    let bob_balances = app.wrap().query_all_balances("bob2").unwrap();
    assert_eq!(bob_balances, coins(45, DENOM));
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(125, DENOM));
    let lucy_balances = app.wrap().query_all_balances("lucy").unwrap();
    assert_eq!(lucy_balances, coins(333, DENOM));

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        600_000 - expected_gone_amount,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr.clone(), &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128));

    // Check balance fully cleared after withdraws
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::AgentWithdraw(None),
        &[],
    )
    .unwrap();
    app.execute_contract(
        factory_addr.clone(),
        manager_addr.clone(),
        &ExecuteMsg::OwnerWithdraw {},
        &[],
    )
    .unwrap();

    let manager_balances = app.wrap().query_all_balances(manager_addr).unwrap();
    assert!(manager_balances.is_empty());
}

#[test]
fn simple_bank_transfers_cron() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    activate_agent(&mut app, &agents_addr);

    let coin_transfer_amount = 45;
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Cron("* * * * * *".to_owned()),
        // Making it cron on purrpose
        boundary: Some(Boundary::Time(BoundaryTime {
            start: Some(app.block_info().time),
            end: Some(app.block_info().time.plus_seconds(20)),
        })),
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: "bob".to_owned(),
                amount: coins(coin_transfer_amount, DENOM),
            }
            .into(),
            gas_limit: None,
        }],
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
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    // Using the task configured fee amounts
    let amt_for_one_task = task_response.task.unwrap().amount_for_one_task;
    let agent_fee = amt_for_one_task.agent_fee;
    let treasury_fee = amt_for_one_task.treasury_fee;
    let gas_price =
        amt_for_one_task.gas_price.numerator as f64 / amt_for_one_task.gas_price.denominator as f64;
    let gas_multiplier = amt_for_one_task.gas_price.gas_adjustment_numerator as f64
        / amt_for_one_task.gas_price.denominator as f64;
    let gas_needed = amt_for_one_task.gas as f64 * gas_multiplier;
    let gas_fees = gas_needed * (agent_fee + treasury_fee) as f64 / 100.0;
    let amount_for_fees = gas_fees * gas_price;
    let expected_gone_amount = {
        let amount_for_task = gas_needed * gas_price; //0.04;
        amount_for_task + amount_for_fees + coin_transfer_amount as f64
    } as u128;

    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();
    app.update_block(add_little_time);
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();
    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // action done
    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    // bob balance is only 1 bank send amount, since second proxy_call is outside boundary
    assert_eq!(bob_balances, coins(coin_transfer_amount, DENOM));

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    let fee_profit = unsafe { amount_for_fees.to_int_unchecked::<u128>() };
    assert_eq!(
        // since there are 2 proxy_call above, we subtract 2 expected_gone_amount
        600_000 - expected_gone_amount - expected_gone_amount,
        // since boundary is exceeded the second time we call proxy_call,
        // need to deduct profit fees for second call
        after_unregister_participant_balance.amount.u128()
            - participant_balance.amount.u128()
            - coin_transfer_amount
            - fee_profit
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    assert_eq!(
        agent_reward,
        Uint128::from(
            ((expected_gone_amount * 2) - fee_profit - (fee_profit / 2))
                - (coin_transfer_amount * 2)
        )
    );

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr.clone(), &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(
        treasury_balance,
        // amount_for_fees is enough for 2 executions, so we adjust
        Uint128::new(amount_for_fees as u128 - (fee_profit / 2))
    );

    // Check manager balances accounts for both agent & treasury
    let manager_balances = app.wrap().query_all_balances(manager_addr.clone()).unwrap();
    assert_eq!(
        manager_balances,
        coins(agent_reward.saturating_add(treasury_balance).into(), DENOM)
    );

    // withdraw rewards so it's clear before second test
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::AgentWithdraw(None),
        &[],
    )
    .unwrap();
    app.execute_contract(
        factory_addr.clone(),
        manager_addr.clone(),
        &ExecuteMsg::OwnerWithdraw {},
        &[],
    )
    .unwrap();

    // Check balance fully cleared
    let manager_balances = app.wrap().query_all_balances(manager_addr.clone()).unwrap();
    assert!(manager_balances.is_empty());

    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Cron("* * * * * *".to_owned()),
        // Making it cron on purpose
        boundary: Some(Boundary::Time(BoundaryTime {
            start: Some(app.block_info().time),
            end: Some(app.block_info().time.plus_seconds(20)),
        })),
        stop_on_fail: false,
        actions: vec![
            Action {
                msg: BankMsg::Send {
                    to_address: "bob2".to_owned(),
                    amount: coins(45, DENOM),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: BankMsg::Send {
                    to_address: "alice".to_owned(),
                    amount: coins(125, DENOM),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: BankMsg::Send {
                    to_address: "lucy".to_owned(),
                    amount: coins(333, DENOM),
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
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let amt_for_one_task = task_response.task.unwrap().amount_for_one_task;
    let agent_fee = amt_for_one_task.agent_fee;
    let treasury_fee = amt_for_one_task.treasury_fee;
    let gas_price =
        amt_for_one_task.gas_price.numerator as f64 / amt_for_one_task.gas_price.denominator as f64;
    let gas_multiplier = amt_for_one_task.gas_price.gas_adjustment_numerator as f64
        / amt_for_one_task.gas_price.denominator as f64;
    let gas_needed = amt_for_one_task.gas as f64 * gas_multiplier;
    let gas_fees = gas_needed * (agent_fee + treasury_fee) as f64 / 100.0;
    let amount_for_fees = gas_fees * gas_price;
    let expected_gone_amount = {
        let amount_for_task = gas_needed * gas_price; //0.04;
        amount_for_task + amount_for_fees + 45.0 + 125.0 + 333.0
    } as u128;

    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();

    // crate::tests::helpers::check_task_chain(&app, &tasks_addr, &agents_addr);

    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();
    app.update_block(add_little_time);
    app.update_block(add_little_time);

    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();
    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // action done
    let bob_balances = app.wrap().query_all_balances("bob2").unwrap();
    assert_eq!(bob_balances, coins(45, DENOM));
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(125, DENOM));
    let lucy_balances = app.wrap().query_all_balances("lucy").unwrap();
    assert_eq!(lucy_balances, coins(333, DENOM));

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    let fee_profit = unsafe { amount_for_fees.to_int_unchecked::<u128>() };
    assert_eq!(
        600_000 - expected_gone_amount - expected_gone_amount,
        after_unregister_participant_balance.amount.u128()
            - participant_balance.amount.u128()
            - (45 + 125 + 333)
            - fee_profit
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    assert_eq!(
        agent_reward,
        Uint128::from(
            ((expected_gone_amount * 2) - fee_profit - (fee_profit / 2)) - ((45 + 125 + 333) * 2)
        )
    );

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr.clone(), &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(
        treasury_balance,
        // amount_for_fees is enough for 2 executions, so we adjust
        Uint128::new(amount_for_fees as u128 - (fee_profit / 2))
    );

    // Check task balance is gone
    let task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(manager_addr.clone(), &QueryMsg::TaskBalance { task_hash })
        .unwrap();
    assert!(task_balance.balance.is_none());

    // Check balance fully clears
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::AgentWithdraw(None),
        &[],
    )
    .unwrap();
    app.execute_contract(
        factory_addr.clone(),
        manager_addr.clone(),
        &ExecuteMsg::OwnerWithdraw {},
        &[],
    )
    .unwrap();

    let manager_balances = app.wrap().query_all_balances(manager_addr).unwrap();
    assert!(manager_balances.is_empty());
}

#[test]
fn multi_coin_bank_transfers() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    activate_agent(&mut app, &agents_addr);

    let coin_transfer_amount: u128 = 321;
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![
            Action {
                msg: BankMsg::Send {
                    to_address: "alice".to_owned(),
                    amount: coins(123, "denom"),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: BankMsg::Send {
                    to_address: "bob".to_owned(),
                    amount: vec![coin(coin_transfer_amount, DENOM), coin(1001, "denom")],
                }
                .into(),
                gas_limit: None,
            },
        ],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let attach_funds = vec![coin(600_000, DENOM), coin(2400, "denom")];
    app.sudo(
        BankSudo::Mint {
            to_address: PARTICIPANT0.to_owned(),
            amount: attach_funds.clone(),
        }
        .into(),
    )
    .unwrap();
    let res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &attach_funds,
        )
        .unwrap();
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let amt_for_one_task = task_response.task.unwrap().amount_for_one_task;
    let agent_fee = amt_for_one_task.agent_fee;
    let treasury_fee = amt_for_one_task.treasury_fee;
    let gas_price =
        amt_for_one_task.gas_price.numerator as f64 / amt_for_one_task.gas_price.denominator as f64;
    let gas_multiplier = amt_for_one_task.gas_price.gas_adjustment_numerator as f64
        / amt_for_one_task.gas_price.denominator as f64;
    let gas_needed = amt_for_one_task.gas as f64 * gas_multiplier;
    let gas_fees = gas_needed * (agent_fee + treasury_fee) as f64 / 100.0;
    let amount_for_fees = gas_fees * gas_price;
    let amount_for_task = gas_needed * gas_price; //0.04;
    let expected_gone_amount =
        { amount_for_task + amount_for_fees + coin_transfer_amount as f64 } as u128;

    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();
    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // action done
    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321, DENOM), coin(1001, "denom")]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123, "denom"));

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        600_000 - expected_gone_amount,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    assert_eq!(
        agent_reward,
        Uint128::from(amount_for_task as u128 + (amount_for_fees as u128 / 2))
    );

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr, &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128 / 2));
}

#[test]
fn cw20_action_transfer() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);
    let cw20_addr = init_cw20(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    // Refill balance
    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        cw20_addr.as_str(),
    );
    app.execute_contract(
        Addr::unchecked(PARTICIPANT0),
        cw20_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: manager_addr.to_string(),
            amount: Uint128::new(555),
            msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    activate_agent(&mut app, &agents_addr);

    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![
            Action {
                msg: WasmMsg::Execute {
                    contract_addr: cw20_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "alice".to_owned(),
                        amount: Uint128::new(500),
                    })
                    .unwrap(),
                    funds: Default::default(),
                }
                .into(),
                gas_limit: Some(250_000),
            },
            Action {
                msg: WasmMsg::Execute {
                    contract_addr: cw20_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "bob".to_owned(),
                        amount: Uint128::new(50),
                    })
                    .unwrap(),
                    funds: Default::default(),
                }
                .into(),
                gas_limit: Some(250_000),
            },
        ],
        queries: None,
        transforms: None,
        cw20: Some(Cw20Coin {
            address: cw20_addr.to_string(),
            amount: Uint128::new(555),
        }),
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
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees
    } as u128;

    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    let participant_cw20_balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: PARTICIPANT0.to_owned(),
            },
        )
        .unwrap();

    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();
    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // action done
    let bob_cw20_balances: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: "bob".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(bob_cw20_balances.balance, Uint128::new(50));
    let bob_cw20_balances: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &Cw20QueryMsg::Balance {
                address: "alice".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(bob_cw20_balances.balance, Uint128::new(500));

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        600_000 - expected_gone_amount,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // unused cw20's returned to the temp
    let participant_cw20_temp_balance: Vec<Cw20CoinVerified> = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::UsersBalances {
                address: PARTICIPANT0.to_owned(),
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        participant_cw20_temp_balance,
        vec![Cw20CoinVerified {
            address: cw20_addr.clone(),
            amount: Uint128::new(5)
        }]
    );
    // And can be withdrawn later
    app.execute_contract(
        Addr::unchecked(PARTICIPANT0),
        manager_addr.clone(),
        &ExecuteMsg::UserWithdraw { limit: None },
        &[],
    )
    .unwrap();
    let after_unregister_participant_cw20_balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr,
            &Cw20QueryMsg::Balance {
                address: PARTICIPANT0.to_owned(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::new(5),
        after_unregister_participant_cw20_balance.balance - participant_cw20_balance.balance
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr, &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128));
}

#[test]
fn task_with_query() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);
    let mod_balances = init_mod_balances(&mut app, &factory_addr);

    activate_agent(&mut app, &agents_addr);

    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: "alice".to_owned(),
                amount: coins(123, DENOM),
            }
            .into(),
            gas_limit: None,
        }],
        queries: Some(vec![CroncatQuery {
            contract_addr: mod_balances.to_string(),
            msg: to_binary(&croncat_mod_balances::msg::QueryMsg::HasBalanceComparator(
                croncat_mod_balances::types::HasBalanceComparator {
                    address: "lucy".to_owned(),
                    required_balance: coins(100, "denom").into(),
                    comparator: croncat_mod_balances::types::BalanceComparator::Eq,
                },
            ))
            .unwrap(),
            check_result: true,
        }]),
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
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees + 123.0
    } as u128;

    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    // Not ready yet
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr.clone(),
            &ExecuteMsg::ProxyCall {
                task_hash: Some(task_hash.clone()),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::TaskQueryResultFalse {});
    // Now let's make it ready!
    app.sudo(
        BankSudo::Mint {
            to_address: "lucy".to_owned(),
            amount: coins(100, "denom"),
        }
        .into(),
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall {
            task_hash: Some(task_hash.clone()),
        },
        &[],
    )
    .unwrap();
    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // action done
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123, DENOM));

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        600_000 - expected_gone_amount,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr.clone(), &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128));

    // repeat to check contract state is progressing as expected
    // first clear up rewards
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::AgentWithdraw(None),
        &[],
    )
    .unwrap();
    app.execute_contract(
        factory_addr.clone(),
        manager_addr.clone(),
        &ExecuteMsg::OwnerWithdraw {},
        &[],
    )
    .unwrap();

    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: "alice".to_owned(),
                amount: coins(123, DENOM),
            }
            .into(),
            gas_limit: None,
        }],
        queries: Some(vec![CroncatQuery {
            contract_addr: mod_balances.to_string(),
            msg: to_binary(&croncat_mod_balances::msg::QueryMsg::HasBalanceComparator(
                croncat_mod_balances::types::HasBalanceComparator {
                    address: "lucy".to_owned(),
                    required_balance: coins(100, "denom").into(),
                    comparator: croncat_mod_balances::types::BalanceComparator::Eq,
                },
            ))
            .unwrap(),
            check_result: true,
        }]),
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
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees + 123.0
    } as u128;

    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();

    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall {
            task_hash: Some(task_hash.clone()),
        },
        &[],
    )
    .unwrap();
    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    // action done
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123 * 2, DENOM));

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        600_000 - expected_gone_amount,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr, &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128));
}

#[test]
fn recurring_task_block_immediate() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    activate_agent(&mut app, &agents_addr);

    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Immediate,
        // repeat it two times
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: None,
            end: Some((app.block_info().height + 1).into()),
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

    // pre action
    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, vec![]);

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
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees + 321.0 + 123.0
    } as u128;

    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    // action done
    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123, DENOM));

    app.update_block(add_little_time);
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321 * 2, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123 * 2, DENOM));

    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        600_000 - expected_gone_amount * 2,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128 * 2;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr.clone(), &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128 * 2));

    // repeat to check contract state is progressing as expected
    // first clear up rewards
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::AgentWithdraw(None),
        &[],
    )
    .unwrap();
    app.execute_contract(
        factory_addr.clone(),
        manager_addr.clone(),
        &ExecuteMsg::OwnerWithdraw {},
        &[],
    )
    .unwrap();

    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Immediate,
        // repeat it two times
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: None,
            end: Some((app.block_info().height + 1).into()),
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
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees + 321.0 + 123.0
    } as u128;

    app.update_block(add_little_time);
    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    // action done
    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321 * 3, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123 * 3, DENOM));

    app.update_block(add_little_time);
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321 * 4, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123 * 4, DENOM));

    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        600_000 - expected_gone_amount * 2,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128 * 2;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr, &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128 * 2));
}

#[test]
fn recurring_task_block_block_interval() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    activate_agent(&mut app, &agents_addr);

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
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees + 321.0 + 123.0
    } as u128;

    // wait 3 blocks
    app.update_block(add_little_time);
    app.update_block(add_little_time);
    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    // action done
    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123, DENOM));

    app.update_block(add_little_time);
    app.update_block(add_little_time);
    app.update_block(add_little_time);

    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321 * 2, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123 * 2, DENOM));

    app.update_block(add_little_time);
    app.update_block(add_little_time);
    app.update_block(add_little_time);

    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321 * 3, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123 * 3, DENOM));

    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        600_000 - expected_gone_amount * 3,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128 * 3;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr, &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128 * 3));
}

#[test]
fn recurring_task_cron() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    activate_agent(&mut app, &agents_addr);

    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Cron("* * * * * *".to_owned()),
        // repeat it two times
        boundary: Some(Boundary::Time(BoundaryTime {
            start: Some(app.block_info().time),
            end: Some(app.block_info().time.plus_seconds(40)),
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
            &coins(75000, DENOM),
        )
        .unwrap();
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees + 321.0 + 123.0
    } as u128;

    app.update_block(add_little_time);

    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    // action done
    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123, DENOM));

    app.update_block(add_little_time);
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321 * 2, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123 * 2, DENOM));

    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        75000 - expected_gone_amount * 2,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128 * 2;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr.clone(), &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128 * 2));

    // repeat to check contract state is progressing as expected
    // first clear up rewards
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::AgentWithdraw(None),
        &[],
    )
    .unwrap();
    app.execute_contract(
        factory_addr.clone(),
        manager_addr.clone(),
        &ExecuteMsg::OwnerWithdraw {},
        &[],
    )
    .unwrap();

    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Cron("* * * * * *".to_owned()),
        // repeat it two times
        boundary: Some(Boundary::Time(BoundaryTime {
            start: Some(app.block_info().time),
            end: Some(app.block_info().time.plus_seconds(40)),
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
            &coins(75000, DENOM),
        )
        .unwrap();
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task {
                task_hash: task_hash.clone(),
            },
        )
        .unwrap();

    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;
    let expected_gone_amount = {
        let gas_fees = gas_needed * (DEFAULT_FEE + DEFAULT_FEE) as f64 / 100.0;
        let amount_for_task = gas_needed * 0.04;
        let amount_for_fees = gas_fees * 0.04;
        amount_for_task + amount_for_fees + 321.0 + 123.0
    } as u128;

    app.update_block(add_little_time);
    let participant_balance = app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    // action done
    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321 * 3, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123 * 3, DENOM));

    app.update_block(add_little_time);
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    let bob_balances = app.wrap().query_all_balances("bob").unwrap();
    assert_eq!(bob_balances, vec![coin(321 * 4, DENOM)]);
    let alice_balances = app.wrap().query_all_balances("alice").unwrap();
    assert_eq!(alice_balances, coins(123 * 4, DENOM));

    // check task got unregistered
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());

    let after_unregister_participant_balance =
        app.wrap().query_balance(PARTICIPANT0, DENOM).unwrap();
    assert_eq!(
        75000 - expected_gone_amount * 2,
        after_unregister_participant_balance.amount.u128() - participant_balance.amount.u128()
    );

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128 * 2;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Check treasury reward
    let treasury_balance: Uint128 = app
        .wrap()
        .query_wasm_smart(manager_addr, &QueryMsg::TreasuryBalance {})
        .unwrap();
    assert_eq!(treasury_balance, Uint128::new(amount_for_fees as u128 * 2));
}

#[test]
fn negative_proxy_call() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);
    let mod_balances = init_mod_balances(&mut app, &factory_addr);

    let cw20_addr = init_cw20(&mut app);
    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        cw20_addr.as_str(),
    );

    app.execute_contract(
        Addr::unchecked(PARTICIPANT0),
        cw20_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: manager_addr.to_string(),
            amount: Uint128::new(555),
            msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    activate_agent(&mut app, &agents_addr);

    // no task for this agent
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr.clone(),
            &ExecuteMsg::ProxyCall { task_hash: None },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NoTaskForAgent {});

    // not registered agent
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT1),
            manager_addr.clone(),
            &ExecuteMsg::ProxyCall { task_hash: None },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NoTaskForAgent {});

    // agent not registered before proxy call with queries
    // Creating task itself
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: WasmMsg::Execute {
                contract_addr: cw20_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "alice".to_owned(),
                    amount: Uint128::new(1),
                })
                .unwrap(),
                funds: Default::default(),
            }
            .into(),
            gas_limit: Some(250_000),
        }],
        queries: Some(vec![CroncatQuery {
            contract_addr: mod_balances.to_string(),
            msg: to_binary(&croncat_mod_balances::msg::QueryMsg::HasBalanceComparator(
                HasBalanceComparator {
                    address: "lucy".to_owned(),
                    required_balance: coins(10, "denom").into(),
                    comparator: croncat_mod_balances::types::BalanceComparator::Eq,
                },
            ))
            .unwrap(),
            check_result: true,
        }]),
        transforms: Some(vec![Transform {
            action_idx: 0,
            query_idx: 0,
            action_path: vec!["transfer".to_owned().into(), "amount".to_owned().into()].into(),
            query_response_path: vec![].into(),
        }]),
        cw20: Some(Cw20Coin {
            address: cw20_addr.to_string(),
            amount: Uint128::new(1),
        }),
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

    app.update_block(add_little_time);

    // Agent not registered
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT1),
            manager_addr.clone(),
            &ExecuteMsg::ProxyCall {
                task_hash: Some(task_hash.clone()),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::AgentNotActive {});

    // Agent not active
    // register agent1 first
    app.execute_contract(
        Addr::unchecked(AGENT1),
        agents_addr.clone(),
        &croncat_sdk_agents::msg::ExecuteMsg::RegisterAgent {
            payable_account_id: None,
        },
        &[],
    )
    .unwrap();
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT1),
            manager_addr.clone(),
            &ExecuteMsg::ProxyCall {
                task_hash: Some(task_hash.clone()),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::AgentNotActive {});

    // active agent(agent0), but task not ready
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr.clone(),
            &ExecuteMsg::ProxyCall {
                task_hash: Some(task_hash.clone()),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::TaskQueryResultFalse {});

    // making task ready
    app.sudo(
        BankSudo::Mint {
            to_address: "lucy".to_owned(),
            amount: coins(10, "denom"),
        }
        .into(),
    )
    .unwrap();

    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr,
            &ExecuteMsg::ProxyCall {
                task_hash: Some(task_hash.clone()),
            },
            &[],
        )
        .unwrap();
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "lifecycle" && attr.value == "task_invalidated")
    }));

    // make sure it's gone after invalidation
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    assert!(task_response.task.is_none());
}

#[test]
fn test_withdraw_agent_fail() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let _tasks_addr = init_tasks(&mut app, &factory_addr);

    // Agent isn't registered
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr.clone(),
            &ExecuteMsg::AgentWithdraw(None),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NoRewardsOwnerAgentFound {});

    activate_agent(&mut app, &agents_addr);

    // No available rewards for withdraw
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr.clone(),
            &ExecuteMsg::AgentWithdraw(None),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NoWithdrawRewardsAvailable {});

    // Unauthorized to withdraw, only agent contracts can call AgentWithdraw with args
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr.clone(),
            &ExecuteMsg::AgentWithdraw(Some(AgentWithdrawOnRemovalArgs {
                agent_id: AGENT0.to_owned(),
                payable_account_id: PARTICIPANT0.to_owned(),
            })),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    // Shouldn't attach funds
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr.clone(),
            &ExecuteMsg::AgentWithdraw(None),
            &[coin(1, DENOM)],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::RedundantFunds {});

    // Paused
    app.execute_contract(
        Addr::unchecked(PAUSE_ADMIN),
        manager_addr.clone(),
        &ExecuteMsg::PauseContract {},
        &[],
    )
    .unwrap();
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr,
            &ExecuteMsg::AgentWithdraw(None),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::ContractPaused {});
}

#[test]
fn test_withdraw_agent_success() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    activate_agent(&mut app, &agents_addr);

    // Create a task
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
    let res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task.clone()),
            },
            &coins(600_000, DENOM),
        )
        .unwrap();

    // Get task info
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();
    let task_response: TaskResponse = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_tasks::msg::QueryMsg::Task { task_hash },
        )
        .unwrap();
    let gas_needed = task_response.task.unwrap().amount_for_one_task.gas as f64 * 1.5;

    app.update_block(add_little_time);

    // Agent executes a task
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    // Check agent reward
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    let gas_fees = gas_needed * DEFAULT_FEE as f64 / 100.0;
    let amount_for_task = gas_needed * 0.04;
    let amount_for_fees = gas_fees * 0.04;
    let expected_agent_reward = (amount_for_task + amount_for_fees) as u128;
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    let agent_balance_before_withdraw = app.wrap().query_balance(AGENT0, DENOM).unwrap().amount;

    // withdraw rewards
    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            manager_addr.clone(),
            &ExecuteMsg::AgentWithdraw(None),
            &[],
        )
        .unwrap();

    let agent_balance_after_withdraw = app.wrap().query_balance(AGENT0, DENOM).unwrap().amount;

    // Check agent balance
    assert_eq!(
        agent_balance_before_withdraw
            .checked_add(agent_reward)
            .unwrap(),
        agent_balance_after_withdraw
    );

    // Check attributes
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "action" && attr.value == "withdraw_rewards")
    }));
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "payment_account_id" && attr.value == *AGENT0)
    }));
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "rewards" && attr.value == agent_reward.to_string())
    }));
    // Check data
    assert_eq!(
        res.data,
        Some(
            to_binary(&AgentWithdrawCallback {
                agent_id: AGENT0.to_string(),
                amount: agent_reward,
                payable_account_id: AGENT0.to_string(),
            })
            .unwrap()
        )
    );

    // Agent balance in manager contract is zero after withdraw
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    assert_eq!(agent_reward, Uint128::zero());

    // Do the same again to check AgentWithdraw with args (when agent contract calls withdraw)

    // Create a task
    app.execute_contract(
        Addr::unchecked(PARTICIPANT0),
        tasks_addr,
        &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
            task: Box::new(task),
        },
        &coins(600_000, DENOM),
    )
    .unwrap();

    app.update_block(add_little_time);

    // Agent executes a task
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    )
    .unwrap();

    // Check agent reward
    // Don't calculate prices again, task is the same
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    assert_eq!(agent_reward, Uint128::from(expected_agent_reward));

    // Agent contract calls withdraw for this agent
    let payable_account_balance_before_withdraw = app
        .wrap()
        .query_balance(PARTICIPANT2, DENOM)
        .unwrap()
        .amount;
    let res = app
        .execute_contract(
            Addr::unchecked(agents_addr.clone()),
            manager_addr.clone(),
            &ExecuteMsg::AgentWithdraw(Some(AgentWithdrawOnRemovalArgs {
                agent_id: AGENT0.to_owned(),
                payable_account_id: PARTICIPANT2.to_owned(),
            })),
            &[],
        )
        .unwrap();
    let payable_account_balance_after_withdraw = app
        .wrap()
        .query_balance(PARTICIPANT2, DENOM)
        .unwrap()
        .amount;

    // Check payable_account balance
    assert_eq!(
        payable_account_balance_before_withdraw
            .checked_add(agent_reward)
            .unwrap(),
        payable_account_balance_after_withdraw
    );

    // Check attributes
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "action" && attr.value == "withdraw_rewards")
    }));
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "payment_account_id" && attr.value == *PARTICIPANT2)
    }));
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "rewards" && attr.value == agent_reward.to_string())
    }));
    // Check data
    assert_eq!(
        res.data,
        Some(
            to_binary(&AgentWithdrawCallback {
                agent_id: AGENT0.to_string(),
                amount: agent_reward,
                payable_account_id: PARTICIPANT2.to_string(),
            })
            .unwrap()
        )
    );

    // Agent balance in manager contract is zero after withdraw
    let agent_reward: Uint128 = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::AgentRewards {
                agent_id: AGENT0.to_owned(),
            },
        )
        .unwrap();
    assert_eq!(agent_reward, Uint128::zero());

    // Agent contract can call AgentWithdraw even if the reward is zero
    let payable_account_balance_before_withdraw = app
        .wrap()
        .query_balance(PARTICIPANT2, DENOM)
        .unwrap()
        .amount;
    // Agent contract calls withdraw for the agent
    let res = app
        .execute_contract(
            Addr::unchecked(agents_addr),
            manager_addr,
            &ExecuteMsg::AgentWithdraw(Some(AgentWithdrawOnRemovalArgs {
                agent_id: AGENT0.to_owned(),
                payable_account_id: PARTICIPANT2.to_owned(),
            })),
            &[],
        )
        .unwrap();
    let payable_account_balance_after_withdraw = app
        .wrap()
        .query_balance(PARTICIPANT2, DENOM)
        .unwrap()
        .amount;

    // Check payable_account balance
    assert_eq!(
        payable_account_balance_before_withdraw,
        payable_account_balance_after_withdraw
    );

    // Check attributes
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "action" && attr.value == "withdraw_rewards")
    }));
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "payment_account_id" && attr.value == *PARTICIPANT2)
    }));
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "rewards" && attr.value == *"0")
    }));
    // Check data
    assert_eq!(
        res.data,
        Some(
            to_binary(&AgentWithdrawCallback {
                agent_id: AGENT0.to_string(),
                amount: Uint128::zero(),
                payable_account_id: PARTICIPANT2.to_string(),
            })
            .unwrap()
        )
    );
}

#[test]
fn refill_task_balance_fail() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let _agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    // RefillTaskBalance with wrong hash
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskBalance {
                task_hash: "hash".to_owned(),
            },
            &coins(100_000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NoTaskHash {});

    // Create a task
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

    app.update_block(add_little_time);

    // RefillTaskBalance called by the wrong address
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT2),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskBalance {
                task_hash: task_hash.to_owned(),
            },
            &coins(100_000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    // RefillTaskBalance with 3 coins
    let attach_funds = vec![coin(100_000, "ujuno"), coin(100_000, "ibc")];
    app.sudo(
        BankSudo::Mint {
            to_address: PARTICIPANT0.to_owned(),
            amount: attach_funds,
        }
        .into(),
    )
    .unwrap();
    let mut participant_balances = app.wrap().query_all_balances(PARTICIPANT0).unwrap();
    assert_eq!(
        participant_balances,
        &[
            coin(4_400_000, DENOM),
            coin(100_000, "ibc"),
            coin(100_000, "ujuno")
        ]
    );

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskBalance {
                task_hash: task_hash.to_owned(),
            },
            &[
                coin(100_000, DENOM),
                coin(10_000, "ujuno".to_owned()),
                coin(10_000, "ibc".to_owned()),
            ],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidAttachedCoins {});

    // RefillTaskBalance with wrong denom, task doesn't have ibc coins
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskBalance {
                task_hash: task_hash.to_owned(),
            },
            &coins(10_000, "ibc".to_owned()),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidAttachedCoins {});

    // Get task balance
    let task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(manager_addr.clone(), &QueryMsg::TaskBalance { task_hash })
        .unwrap();
    assert_eq!(
        task_balance.balance.unwrap(),
        TaskBalance {
            native_balance: 600_000u64.into(),
            cw20_balance: None,
            ibc_balance: None
        }
    );

    // Check that PARTICIPANT0 balance didn't change
    participant_balances = app.wrap().query_all_balances(PARTICIPANT0).unwrap();
    assert_eq!(
        participant_balances,
        &[
            coin(4_400_000, DENOM),
            coin(100_000, "ibc"),
            coin(100_000, "ujuno")
        ]
    );

    // Create task without ibc balance, but attach some, should fail
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: "bob".to_owned(),
                amount: coins(46, DENOM),
            }
            .into(),
            gas_limit: None,
        }],
        queries: None,
        transforms: None,
        cw20: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &[coin(600_000, DENOM), coin(50_000, "ibc")],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::Sdk(croncat_sdk_manager::SdkError::NonRequiredDenom {})
    );

    // Create task with ibc balance
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![
            Action {
                msg: BankMsg::Send {
                    to_address: "bob".to_owned(),
                    amount: coins(46, DENOM),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: BankMsg::Send {
                    to_address: "bob".to_owned(),
                    amount: coins(5_000, "ibc"),
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
            tasks_addr,
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &[coin(600_000, DENOM), coin(50_000, "ibc")],
        )
        .unwrap();
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();

    // Check PARTICIPANT0 balance
    participant_balances = app.wrap().query_all_balances(PARTICIPANT0).unwrap();
    assert_eq!(
        participant_balances,
        &[
            coin(3_800_000, DENOM),
            coin(50_000, "ibc"),
            coin(100_000, "ujuno")
        ]
    );

    // RefillTaskBalance with wrong denom, task has ibc coins
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskBalance {
                task_hash: task_hash.to_owned(),
            },
            &coins(10_000, "ujuno".to_owned()),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidAttachedCoins {});

    // Pause
    app.execute_contract(
        Addr::unchecked(PAUSE_ADMIN),
        manager_addr.clone(),
        &ExecuteMsg::PauseContract {},
        &[],
    )
    .unwrap();

    // RefillTaskBalance when contract is paused
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT2),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskBalance {
                task_hash: task_hash.to_owned(),
            },
            &coins(100_000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::ContractPaused {});

    // Check task balance
    let task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(manager_addr, &QueryMsg::TaskBalance { task_hash })
        .unwrap();
    assert_eq!(
        task_balance.balance.unwrap(),
        TaskBalance {
            native_balance: 600_000u64.into(),
            cw20_balance: None,
            ibc_balance: Some(coin(50_000, "ibc".to_owned()))
        }
    );

    // Check that PARTICIPANT0 balance didn't change
    participant_balances = app.wrap().query_all_balances(PARTICIPANT0).unwrap();
    assert_eq!(
        participant_balances,
        &[
            coin(3_800_000, DENOM),
            coin(50_000, "ibc"),
            coin(100_000, "ujuno")
        ]
    );
}

#[test]
fn refill_task_balance_success() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let _agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    // Create task with ibc balance
    let attach_funds = vec![coin(100_000, "ibc")];
    app.sudo(
        BankSudo::Mint {
            to_address: PARTICIPANT0.to_owned(),
            amount: attach_funds,
        }
        .into(),
    )
    .unwrap();
    let mut participant_balances = app.wrap().query_all_balances(PARTICIPANT0).unwrap();
    assert_eq!(
        participant_balances,
        &[coin(5_000_000, DENOM), coin(100_000, "ibc")]
    );

    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![
            Action {
                msg: BankMsg::Send {
                    to_address: "bob".to_owned(),
                    amount: coins(45, DENOM),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: BankMsg::Send {
                    to_address: "bob".to_owned(),
                    amount: coins(5_000, "ibc"),
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
            tasks_addr,
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &[coin(600_000, DENOM), coin(50_000, "ibc")],
        )
        .unwrap();
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();

    // PARTICIPANT0 balances
    participant_balances = app.wrap().query_all_balances(PARTICIPANT0).unwrap();
    assert_eq!(
        participant_balances,
        &[coin(4_400_000, DENOM), coin(50_000, "ibc")]
    );

    app.update_block(add_little_time);

    // RefillTaskBalance with native coins
    let res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskBalance {
                task_hash: task_hash.to_owned(),
            },
            &coins(100_000, DENOM),
        )
        .unwrap();

    // Check attributes
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "action" && attr.value == "refill_native_balance")
    }));

    // Check task balance
    let task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.clone(),
            &QueryMsg::TaskBalance {
                task_hash: task_hash.to_owned(),
            },
        )
        .unwrap();
    assert_eq!(
        task_balance.balance.unwrap(),
        TaskBalance {
            native_balance: 700_000u64.into(),
            cw20_balance: None,
            ibc_balance: Some(coin(50_000, "ibc".to_owned()))
        }
    );

    // Check PARTICIPANT0 balances
    participant_balances = app.wrap().query_all_balances(PARTICIPANT0).unwrap();
    assert_eq!(
        participant_balances,
        &[coin(4_300_000, DENOM), coin(50_000, "ibc")]
    );

    // RefillTaskBalance with ibc coins
    let res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskBalance {
                task_hash: task_hash.to_owned(),
            },
            &coins(30_000, "ibc"),
        )
        .unwrap();

    // Check attributes
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "action" && attr.value == "refill_native_balance")
    }));

    // Check task balance
    let task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(manager_addr, &QueryMsg::TaskBalance { task_hash })
        .unwrap();
    assert_eq!(
        task_balance.balance.unwrap(),
        TaskBalance {
            native_balance: 700_000u64.into(),
            cw20_balance: None,
            ibc_balance: Some(coin(80_000, "ibc".to_owned()))
        }
    );

    // Check PARTICIPANT0 balances
    participant_balances = app.wrap().query_all_balances(PARTICIPANT0).unwrap();
    assert_eq!(
        participant_balances,
        &[coin(4_300_000, DENOM), coin(20_000, "ibc")]
    );
}

#[test]
fn refill_task_cw20_fail() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let _agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    let cw20_addr = init_cw20(&mut app);
    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        cw20_addr.as_str(),
    );

    let cw20 = Cw20Coin {
        address: cw20_addr.to_string(),
        amount: 100u64.into(),
    };

    // RefillTaskCw20Balance with wrong hash
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskCw20Balance {
                task_hash: "hash".to_owned(),
                cw20: cw20.clone(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NoTaskHash {});

    // Create a task
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

    app.update_block(add_little_time);

    // RefillTaskCw20Balance with funds
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskCw20Balance {
                task_hash: task_hash.to_owned(),
                cw20: cw20.clone(),
            },
            &coins(100_000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::RedundantFunds {});

    // RefillTaskCw20Balance called by the wrong address
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT2),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskCw20Balance {
                task_hash: task_hash.to_owned(),
                cw20: cw20.clone(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    // RefillTaskBalance fails because PARTICIPANT0 doesn't have cw20 deposit
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskCw20Balance {
                task_hash: task_hash.to_owned(),
                cw20: cw20.clone(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::EmptyBalance {});

    // PARTICIPANT0 deposits some cw20 coins
    app.execute_contract(
        Addr::unchecked(PARTICIPANT0),
        cw20_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: manager_addr.to_string(),
            amount: Uint128::new(555),
            msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
        },
        &[],
    )
    .unwrap();
    let mut wallet_balances = query_users_manager(&app, &manager_addr, PARTICIPANT0);
    assert_eq!(
        wallet_balances,
        vec![Cw20CoinVerified {
            address: cw20_addr.to_owned(),
            amount: Uint128::new(555),
        }]
    );

    // RefillTaskCw20Balance fails because the task balance doesn't have cw20
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskCw20Balance {
                task_hash: task_hash.to_owned(),
                cw20: cw20.clone(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidAttachedCoins {});

    // Get task balance
    let task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(manager_addr.clone(), &QueryMsg::TaskBalance { task_hash })
        .unwrap();
    assert_eq!(
        task_balance.balance.unwrap(),
        TaskBalance {
            native_balance: 600_000u64.into(),
            cw20_balance: None,
            ibc_balance: None
        }
    );

    // PARTICIPANT0 cw20 deposit didn't change
    wallet_balances = query_users_manager(&app, &manager_addr, PARTICIPANT0);
    assert_eq!(
        wallet_balances,
        vec![Cw20CoinVerified {
            address: cw20_addr.to_owned(),
            amount: Uint128::new(555),
        }]
    );

    // Create a task without a cw20, attach some anyway
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![Action {
            msg: BankMsg::Send {
                to_address: "bob".to_owned(),
                amount: coins(46, DENOM),
            }
            .into(),
            gas_limit: None,
        }],
        queries: None,
        transforms: None,
        cw20: Some(cw20.clone()),
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::Sdk(croncat_sdk_manager::SdkError::NonRequiredDenom {})
    );

    // Create a task with cw20
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![
            Action {
                msg: BankMsg::Send {
                    to_address: "bob".to_owned(),
                    amount: coins(46, DENOM),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: WasmMsg::Execute {
                    contract_addr: cw20_addr.clone().to_string(),
                    msg: to_binary(&cw20::Cw20ExecuteMsg::Send {
                        contract: manager_addr.to_string(),
                        amount: Uint128::new(55),
                        msg: Binary::default(),
                    })
                    .unwrap(),
                    funds: vec![],
                }
                .into(),
                gas_limit: Some(90_000),
            },
        ],
        queries: None,
        transforms: None,
        cw20: Some(cw20.clone()),
    };
    let res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr,
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap();
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();

    // PARTICIPANT0 spent 100 coins
    wallet_balances = query_users_manager(&app, &manager_addr, PARTICIPANT0);
    assert_eq!(
        wallet_balances,
        vec![Cw20CoinVerified {
            address: cw20_addr.to_owned(),
            amount: Uint128::new(455),
        }]
    );

    app.update_block(add_little_time);

    // Try RefillTaskCw20Balance with wrong cw20 address
    let new_cw20_addr = init_cw20(&mut app);
    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        new_cw20_addr.as_str(),
    );
    app.execute_contract(
        Addr::unchecked(PARTICIPANT0),
        new_cw20_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: manager_addr.to_string(),
            amount: Uint128::new(555),
            msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Check PARTICIPANT0 cw20 deposit on manager contract
    wallet_balances = query_users_manager(&app, &manager_addr, PARTICIPANT0);
    assert_eq!(
        wallet_balances,
        vec![
            Cw20CoinVerified {
                address: cw20_addr.to_owned(),
                amount: Uint128::new(455),
            },
            Cw20CoinVerified {
                address: new_cw20_addr.to_owned(),
                amount: Uint128::new(555),
            }
        ]
    );
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskCw20Balance {
                task_hash: task_hash.to_owned(),
                cw20: Cw20Coin {
                    address: new_cw20_addr.to_string(),
                    amount: 1u64.into(),
                },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidAttachedCoins {});

    // Pause
    app.execute_contract(
        Addr::unchecked(PAUSE_ADMIN),
        manager_addr.clone(),
        &ExecuteMsg::PauseContract {},
        &[],
    )
    .unwrap();

    // RefillTaskBalance when contract is paused
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskCw20Balance {
                task_hash: task_hash.to_owned(),
                cw20,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::ContractPaused {});

    // Get task balance
    let task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.to_owned(),
            &QueryMsg::TaskBalance { task_hash },
        )
        .unwrap();
    assert_eq!(
        task_balance.balance.unwrap(),
        TaskBalance {
            native_balance: 600_000u64.into(),
            cw20_balance: Some(Cw20CoinVerified {
                address: cw20_addr.to_owned(),
                amount: 100u64.into()
            }),
            ibc_balance: None
        }
    );

    // Check that PARTICIPANT0 cw20 balance didn't change
    wallet_balances = query_users_manager(&app, &manager_addr, PARTICIPANT0);
    assert_eq!(
        wallet_balances,
        vec![
            Cw20CoinVerified {
                address: cw20_addr,
                amount: Uint128::new(455),
            },
            Cw20CoinVerified {
                address: new_cw20_addr,
                amount: Uint128::new(555),
            }
        ]
    );
}

#[test]
fn refill_task_cw20_success() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let _agents_addr = init_agents(&mut app, &factory_addr);
    let tasks_addr = init_tasks(&mut app, &factory_addr);

    let cw20_addr = init_cw20(&mut app);
    support_new_cw20(
        &mut app,
        factory_addr.clone(),
        &manager_addr,
        cw20_addr.as_str(),
    );

    let cw20 = Cw20Coin {
        address: cw20_addr.to_string(),
        amount: 100u64.into(),
    };

    // PARTICIPANT0 deposits some cw20 coins
    app.execute_contract(
        Addr::unchecked(PARTICIPANT0),
        cw20_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: manager_addr.to_string(),
            amount: Uint128::new(555),
            msg: to_binary(&ReceiveMsg::RefillTempBalance {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Check the deposit
    let mut wallet_balances = query_users_manager(&app, &manager_addr, PARTICIPANT0);
    assert_eq!(
        wallet_balances,
        vec![Cw20CoinVerified {
            address: cw20_addr.to_owned(),
            amount: Uint128::new(555),
        }]
    );

    // Create a task with cw20
    let task = croncat_sdk_tasks::types::TaskRequest {
        interval: Interval::Once,
        boundary: None,
        stop_on_fail: false,
        actions: vec![
            Action {
                msg: BankMsg::Send {
                    to_address: "bob".to_owned(),
                    amount: coins(45, DENOM),
                }
                .into(),
                gas_limit: None,
            },
            Action {
                msg: WasmMsg::Execute {
                    contract_addr: cw20_addr.clone().to_string(),
                    msg: to_binary(&cw20::Cw20ExecuteMsg::Send {
                        contract: manager_addr.to_string(),
                        amount: Uint128::new(55),
                        msg: Binary::default(),
                    })
                    .unwrap(),
                    funds: vec![],
                }
                .into(),
                gas_limit: Some(90_000),
            },
        ],
        queries: None,
        transforms: None,
        cw20: Some(cw20.clone()),
    };
    let res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            tasks_addr,
            &croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask {
                task: Box::new(task),
            },
            &coins(600_000, DENOM),
        )
        .unwrap();
    let task_hash = String::from_vec(res.data.unwrap().0).unwrap();

    // PARTICIPANT0 spent 455 coins
    wallet_balances = query_users_manager(&app, &manager_addr, PARTICIPANT0);
    assert_eq!(
        wallet_balances,
        vec![Cw20CoinVerified {
            address: cw20_addr.to_owned(),
            amount: Uint128::new(455),
        }]
    );

    app.update_block(add_little_time);

    // Refill task balance
    let res = app
        .execute_contract(
            Addr::unchecked(PARTICIPANT0),
            manager_addr.clone(),
            &ExecuteMsg::RefillTaskCw20Balance {
                task_hash: task_hash.to_owned(),
                cw20,
            },
            &[],
        )
        .unwrap();

    // Get task balance, cw20_balance increased
    let task_balance: TaskBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            manager_addr.to_owned(),
            &QueryMsg::TaskBalance { task_hash },
        )
        .unwrap();
    assert_eq!(
        task_balance.balance.unwrap(),
        TaskBalance {
            native_balance: 600_000u64.into(),
            cw20_balance: Some(Cw20CoinVerified {
                address: cw20_addr.clone(),
                amount: 200u64.into()
            }),
            ibc_balance: None
        }
    );

    // PARTICIPANT0 balance decreased
    wallet_balances = query_users_manager(&app, &manager_addr, PARTICIPANT0);
    assert_eq!(
        wallet_balances,
        vec![Cw20CoinVerified {
            address: cw20_addr.to_owned(),
            amount: Uint128::new(355),
        }]
    );

    // Check attributes
    assert!(res.events.iter().any(|ev| {
        ev.attributes
            .iter()
            .any(|attr| attr.key == "action" && attr.value == "refill_task_cw20")
    }));
    assert!(res.events.iter().any(|ev| {
        ev.attributes.iter().any(|attr| {
            attr.key == "cw20_refilled"
                && attr.value
                    == Cw20CoinVerified {
                        address: cw20_addr.clone(),
                        amount: 100u64.into(),
                    }
                    .to_string()
        })
    }));
    assert!(res.events.iter().any(|ev| {
        ev.attributes.iter().any(|attr| {
            attr.key == "task_cw20_balance"
                && attr.value
                    == Cw20CoinVerified {
                        address: cw20_addr.clone(),
                        amount: 200u64.into(),
                    }
                    .to_string()
        })
    }));
}

#[test]
fn scheduled_task_with_boundary_issue() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    // let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let tasks_addr = init_tasks(&mut app, &factory_addr);
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agent_addr = init_agents(&mut app, &factory_addr);

    // Register an agent
    app.execute_contract(
        Addr::unchecked(AGENT0),
        agent_addr,
        &RegisterAgent {
            payable_account_id: None,
        },
        &[],
    )
    .expect("Could not register agent");

    // Create a Once task with a Boundary that is soon
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: None,
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
    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &CreateTask {
            task: Box::new(task),
        },
        &coins(50_000, DENOM),
    )
    .expect("Couldn't create task");

    app.update_block(|block| add_seconds_to_block(block, 120));
    app.update_block(|block| increment_block_height(block, Some(20)));

    // Have agent call proxy call, and check how it went

    let proxy_call_res = app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr,
        &ProxyCall { task_hash: None },
        &[], // Attach no funds
    );

    // The ProxyCall with "succeed" but the task should be ended,
    // and when it's ended, we can look for a specific Attribute to
    // be included in the response.
    let target_attribute = Attribute::new("lifecycle", "task_ended");
    let has_task_ended_attr = proxy_call_res
        .unwrap()
        .events
        .iter()
        .any(|event| event.attributes.contains(&target_attribute));
    assert!(
        has_task_ended_attr,
        "Did not see the lifecycle returned explaining that the task ended"
    );

    // Check number of regular tasks (should be zero since our task should be ended and gone)
    let tasks_for_agent: Vec<croncat_sdk_tasks::types::TaskInfo> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr,
            &croncat_sdk_tasks::msg::TasksQueryMsg::Tasks {
                from_index: None,
                limit: None,
            },
        )
        .expect("Error unwrapping regular task query");
    assert_eq!(
        tasks_for_agent.len(),
        0usize,
        "Should have no regular tasks since it ended"
    );
}

#[test]
fn event_task_with_boundary_issue() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    // let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let tasks_addr = init_tasks(&mut app, &factory_addr);
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agent_addr = init_agents(&mut app, &factory_addr);

    // Register an agent
    app.execute_contract(
        Addr::unchecked(AGENT0),
        agent_addr,
        &RegisterAgent {
            payable_account_id: None,
        },
        &[],
    )
    .expect("Could not register agent");

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

    // Create a task (queries and transforms) with a Boundary that is soon
    let task = TaskRequest {
        interval: Interval::Once,
        boundary: Some(Boundary::Height(BoundaryHeight {
            start: None,
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
        queries: Some(queries),
        transforms: Some(transforms),
        cw20: None,
    };

    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr,
        &CreateTask {
            task: Box::new(task),
        },
        &coins(500_000, DENOM),
    )
    .expect("Couldn't create task");

    app.update_block(|block| add_seconds_to_block(block, 120));
    app.update_block(|block| increment_block_height(block, Some(20)));

    // Have agent call proxy call, and check how it went
    let proxy_call_res = app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr,
        &ProxyCall { task_hash: None },
        &[], // Attach no funds
    );
    assert!(
        proxy_call_res.is_err(),
        "Expecting proxy_call to error because task is no longer valid"
    );
    let contract_error: ContractError = proxy_call_res.unwrap_err().downcast().unwrap();
    assert_eq!(contract_error, ContractError::NoTaskForAgent {});
}

pub(crate) fn add_seconds_to_block(block: &mut BlockInfo, seconds: u64) {
    block.time = block.time.plus_seconds(seconds);
}
pub(crate) fn increment_block_height(block: &mut BlockInfo, inc_value: Option<u64>) {
    block.height += inc_value.unwrap_or(1);
}

/// Checks that the creation of a task with a query (that has check_result = true)
/// and evaluates false, will not be executed by proxy call if the agent
/// provides the task_hash.
#[test]
fn event_task_with_failed_check_result() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    // let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let tasks_addr = init_tasks(&mut app, &factory_addr);
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agent_addr = init_agents(&mut app, &factory_addr);
    let boolean_addr = init_boolean(&mut app);

    // Register an agent
    app.execute_contract(
        Addr::unchecked(AGENT0),
        agent_addr,
        &RegisterAgent {
            payable_account_id: None,
        },
        &[],
    )
    .expect("Could not register agent");

    let queries = vec![CroncatQuery {
        contract_addr: boolean_addr.to_string(),
        // Calls `get_value` on boolean contract, which defaults to false
        msg: to_binary(&cw_boolean_contract::msgs::query_msg::QueryMsg::GetValue {}).unwrap(),
        // It's important to set this to true
        check_result: true,
    }];

    // Create a task (queries and transforms) with a Boundary that is soon
    let task = TaskRequest {
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
        // queries: None,
        queries: Some(queries),
        transforms: None, // No transforms in this task
        cw20: None,
    };

    app.execute_contract(
        Addr::unchecked(ANYONE),
        tasks_addr.clone(),
        &CreateTask {
            task: Box::new(task),
        },
        &coins(500_000, DENOM),
    )
    .expect("Couldn't create task");

    // Agent checks to see if there are tasks for them to do.
    // Note: we hit the Tasks contract for this one. Manager for regular tasks.
    let mut tasks_for_agent: Option<Vec<croncat_sdk_tasks::types::TaskInfo>> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksQueryMsg::EventedTasks {
                start: None,
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    let mut evented_task_info = tasks_for_agent.expect("Error unwrapping evented task info");
    assert_eq!(
        evented_task_info.len(),
        1usize,
        "Should only have one evented task"
    );

    // Have agent call proxy call (without hash), and check how it went
    let mut proxy_call_res = app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &ProxyCall { task_hash: None },
        &[], // Attach no funds
    );
    let contract_error: ContractError = proxy_call_res.unwrap_err().downcast().unwrap();
    // Should be no "regular" tasks, only the one evented one
    assert_eq!(contract_error, ContractError::NoTaskForAgent {});

    // Now check that the evented one returns false and doesn't complete
    let task_hash: String = evented_task_info[0].clone().task_hash;
    proxy_call_res = app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr,
        &ProxyCall {
            task_hash: Some(task_hash),
        },
        &[], // Attach no funds ofc
    );
    assert!(
        proxy_call_res.is_err(),
        "Proxy call should fail since the check_return comes back false"
    );
    let proxy_call_err: ContractError = proxy_call_res.unwrap_err().downcast().unwrap();
    assert_eq!(proxy_call_err, ContractError::TaskQueryResultFalse {});

    // Check that the one tasks still exists
    tasks_for_agent = app
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
    evented_task_info =
        tasks_for_agent.expect("Error unwrapping evented task info after failed proxy_call");
    assert_eq!(
        evented_task_info.len(),
        1usize,
        "Should still have one evented task"
    );
}

/// Checks that the creation of a task with an Immediate interval
/// is able to execute multiple times when given sufficient funds
#[test]
fn immediate_event_task_has_multiple_executions() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);

    // let instantiate_msg: InstantiateMsg = default_instantiate_msg();
    let instantiate_msg: InstantiateMsg = default_instantiate_message();
    let tasks_addr = init_tasks(&mut app, &factory_addr);
    let manager_addr = init_manager(&mut app, &instantiate_msg, &factory_addr, &[]);
    let agent_addr = init_agents(&mut app, &factory_addr);
    let boolean_addr = init_boolean(&mut app);

    // Set boolean contract to return true
    app.execute_contract(
        Addr::unchecked(ANYONE),
        boolean_addr.clone(),
        &Toggle {},
        &[],
    )
    .expect("Toggling boolean contract from false to true failed");

    // Register an agent
    app.execute_contract(
        Addr::unchecked(AGENT0),
        agent_addr,
        &RegisterAgent {
            payable_account_id: None,
        },
        &[],
    )
    .expect("Could not register agent");

    let queries = vec![CroncatQuery {
        contract_addr: boolean_addr.to_string(),
        // Calls `get_value` on boolean contract, which defaults to false
        msg: to_binary(&cw_boolean_contract::msgs::query_msg::QueryMsg::GetValue {}).unwrap(),
        // It's important to set this to true
        check_result: true,
    }];

    // Create a task (queries and transforms) with a Boundary that is soon
    let task = TaskRequest {
        interval: Interval::Immediate,
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
        queries: Some(queries),
        transforms: None, // No transforms in this task
        cw20: None,
    };

    let _res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            tasks_addr.clone(),
            &CreateTask {
                task: Box::new(task),
            },
            &coins(126_740, DENOM),
        )
        .expect("Couldn't create task");

    // Agent checks to see if there are tasks for them to do.
    // Note: we hit the Tasks contract for this one. Manager for regular tasks.
    let mut tasks_for_agent: Option<Vec<croncat_sdk_tasks::types::TaskInfo>> = app
        .wrap()
        .query_wasm_smart(
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksQueryMsg::EventedTasks {
                start: None,
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    let mut evented_task_info = tasks_for_agent.expect("Error unwrapping evented task info");
    assert_eq!(
        &evented_task_info.len(),
        &1usize,
        "Should have one evented task"
    );
    let task_hash = evented_task_info[0].clone().task_hash;

    let proxy_call_msg = ProxyCall {
        task_hash: Some(task_hash),
    };
    // Have agent call proxy call (without hash), and check how it went
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr.clone(),
        &proxy_call_msg,
        &[], // Attach no funds
    )
    .expect("First proxy call should succeed");

    // Check that the one Immediate tasks still exists
    tasks_for_agent = app
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
    evented_task_info =
        tasks_for_agent.expect("Error unwrapping evented task info after successful proxy_call");
    assert_eq!(
        evented_task_info.len(),
        1usize,
        "Should still have one evented task"
    );

    // Call ProxyCall again, expecting it to succeed
    app.execute_contract(
        Addr::unchecked(AGENT0),
        manager_addr,
        &proxy_call_msg,
        &[], // Attach no funds
    )
    .expect("Second proxy call should succeed");
}

#[test]
fn config_invalid_percentage_updates() {
    let mut app = default_app();
    let factory_addr = init_factory(&mut app);
    let manager_addr = init_manager(&mut app, &default_instantiate_message(), &factory_addr, &[]);

    // Check that agent_fee of 101 (above 100%) is invalid
    let mut update_cfg_msg = UpdateConfig {
        agent_fee: Some(10_001), // Above 10_000
        treasury_fee: Some(0),
        gas_price: Some(GasPrice {
            numerator: 555,
            denominator: 666,
            gas_adjustment_numerator: 777,
        }),
        croncat_tasks_key: Some(("new_key_tasks".to_owned(), [0, 1])),
        croncat_agents_key: Some(("new_key_agents".to_owned(), [0, 1])),
        treasury_addr: Some(ANYONE.to_owned()),
        cw20_whitelist: Some(vec!["randomcw20".to_owned()]),
    };

    let mut err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryExecuteMsg::Proxy {
                msg: WasmMsg::Execute {
                    contract_addr: manager_addr.to_string(),
                    msg: to_binary(&ExecuteMsg::UpdateConfig(Box::new(update_cfg_msg.clone())))
                        .unwrap(),
                    funds: vec![],
                },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::InvalidPercentage {
            field: "agent_fee".to_string()
        }
    );

    // Now check the same for the treasury_fee
    update_cfg_msg.agent_fee = Some(5);
    update_cfg_msg.treasury_fee = Some(22_222); // Above 10_000

    err = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryExecuteMsg::Proxy {
                msg: WasmMsg::Execute {
                    contract_addr: manager_addr.to_string(),
                    msg: to_binary(&ExecuteMsg::UpdateConfig(Box::new(update_cfg_msg))).unwrap(),
                    funds: vec![],
                },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::InvalidPercentage {
            field: "treasury_fee".to_string()
        }
    );
}

/// Check for instantiate pause admin scenarios of pass/fail
/// Check for pause & unpause scenarios of pass/fail
#[test]
fn pause_admin_cases() {
    let mut app = default_app();

    let factory_code_id = app.store_code(contracts::croncat_factory_contract());
    let manager_code_id = app.store_code(contracts::croncat_manager_contract());

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

    let init_manager_contract_msg = InstantiateMsg {
        version: Some("0.1".to_owned()),
        croncat_tasks_key: (AGENT1.to_owned(), [0, 1]),
        croncat_agents_key: (AGENT2.to_owned(), [0, 1]),
        pause_admin: Addr::unchecked(PAUSE_ADMIN),
        gas_price: Some(GasPrice {
            numerator: 10,
            denominator: 20,
            gas_adjustment_numerator: 30,
        }),
        treasury_addr: Some(AGENT2.to_owned()),
        cw20_whitelist: Some(vec![PARTICIPANT3.to_owned()]),
    };
    // Attempt to initialize with short address for pause_admin
    let mut init_manager_contract_msg_short_addr = init_manager_contract_msg.clone();
    init_manager_contract_msg_short_addr.pause_admin = Addr::unchecked(ANYONE);
    // Attempt to initialize with same owner address for pause_admin
    let mut init_manager_contract_msg_same_owner = init_manager_contract_msg.clone();
    init_manager_contract_msg_same_owner.pause_admin = Addr::unchecked(ADMIN);

    // Should fail: shorty addr
    let manager_module_instantiate_info = croncat_sdk_factory::msg::ModuleInstantiateInfo {
        code_id: manager_code_id,
        version: [0, 1],
        commit_id: "some".to_owned(),
        checksum: "qwe123".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&init_manager_contract_msg_short_addr).unwrap(),
        contract_name: "manager".to_owned(),
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            croncat_factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryExecuteMsg::Deploy {
                kind: croncat_sdk_factory::msg::VersionKind::Manager,
                module_instantiate_info: manager_module_instantiate_info,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidPauseAdmin {});

    // Should fail: same as owner
    let manager_module_instantiate_info = croncat_sdk_factory::msg::ModuleInstantiateInfo {
        code_id: manager_code_id,
        version: [0, 1],
        commit_id: "some".to_owned(),
        checksum: "qwe123".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&init_manager_contract_msg_same_owner).unwrap(),
        contract_name: "manager".to_owned(),
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            croncat_factory_addr.clone(),
            &croncat_sdk_factory::msg::FactoryExecuteMsg::Deploy {
                kind: croncat_sdk_factory::msg::VersionKind::Manager,
                module_instantiate_info: manager_module_instantiate_info,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidPauseAdmin {});

    // Now, we do a working furr shurr case
    let manager_module_instantiate_info = croncat_sdk_factory::msg::ModuleInstantiateInfo {
        code_id: manager_code_id,
        version: [0, 1],
        commit_id: "some".to_owned(),
        checksum: "qwe123".to_owned(),
        changelog_url: None,
        schema: None,
        msg: to_binary(&init_manager_contract_msg).unwrap(),
        contract_name: "manager".to_owned(),
    };

    // Successfully deploy agents contract
    app.execute_contract(
        Addr::unchecked(ADMIN),
        croncat_factory_addr.clone(),
        &croncat_sdk_factory::msg::FactoryExecuteMsg::Deploy {
            kind: croncat_sdk_factory::msg::VersionKind::Manager,
            module_instantiate_info: manager_module_instantiate_info,
        },
        &[],
    )
    .unwrap();

    // Get agents contract address
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
    let manager_metadata = manager_contracts.metadata.unwrap();
    let croncat_manager_addr = manager_metadata.contract_addr;

    // Owner Should not be able to pause, not pause_admin
    let error: ContractError = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            croncat_manager_addr.clone(),
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
            croncat_manager_addr.clone(),
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
        croncat_manager_addr.clone(),
        &ExecuteMsg::PauseContract {},
        &[],
    );
    assert!(res.is_ok());

    // Check the pause query is valid
    let is_paused: bool = app
        .wrap()
        .query_wasm_smart(croncat_manager_addr.clone(), &QueryMsg::Paused {})
        .unwrap();
    assert!(is_paused);

    // Pause Admin Should not be able to unpause
    let error: ContractError = app
        .execute_contract(
            Addr::unchecked(PAUSE_ADMIN),
            croncat_manager_addr.clone(),
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
            croncat_manager_addr.clone(),
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
                contract_addr: croncat_manager_addr.to_string(),
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
        .query_wasm_smart(croncat_manager_addr, &QueryMsg::Paused {})
        .unwrap();
    assert!(!is_paused);
}
