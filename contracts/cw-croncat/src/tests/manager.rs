use crate::contract::GAS_BASE_FEE_JUNO;
use crate::tests::helpers::{add_little_time, add_one_duration_of_time, proper_instantiate};
use crate::ContractError;
use cosmwasm_std::{
    coin, coins, to_binary, Addr, BankMsg, Coin, CosmosMsg, StakingMsg, StdResult, Uint128, WasmMsg,
};
use cw_croncat_core::msg::{
    AgentTaskResponse, ExecuteMsg, QueryMsg, TaskRequest, TaskResponse, TaskWithRulesResponse,
};
use cw_croncat_core::types::{Action, Boundary, Interval};
use cw_multi_test::Executor;
use cw_rules_core::types::{HasBalanceGte, Rule};

use super::helpers::{ADMIN, AGENT0, AGENT_BENEFICIARY, ANYONE, NATIVE_DENOM};

#[test]
fn proxy_call_fail_cases() -> StdResult<()> {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();
    let proxy_call_msg = ExecuteMsg::ProxyCall { task_hash: None };
    let validator = String::from("you");
    let amount = coin(3, NATIVE_DENOM);
    let stake = StakingMsg::Delegate { validator, amount };
    let msg: CosmosMsg = stake.clone().into();

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: Some(Boundary::Height {
                start: None,
                end: None,
            }),
            stop_on_fail: false,
            actions: vec![Action {
                msg,
                gas_limit: Some(150_000),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };
    let task_id_str =
        "95c916a53fa9d26deef094f7e1ee31c00a2d47b8bf474b2e06d39aebfb1fecc7".to_string();

    // Must attach funds
    let res_err = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            contract_addr.clone(),
            &proxy_call_msg,
            &coins(300010, NATIVE_DENOM),
        )
        .unwrap_err();
    assert_eq!(
        ContractError::CustomError {
            val: "Must not attach funds".to_string()
        },
        res_err.downcast().unwrap()
    );

    // Create task paused
    let change_settings_msg = ExecuteMsg::UpdateSettings {
        paused: Some(true),
        owner_id: None,
        // treasury_id: None,
        agent_fee: None,
        min_tasks_per_agent: None,
        agents_eject_threshold: None,
        gas_price: None,
        proxy_callback_gas: None,
        slot_granularity: None,
    };
    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &change_settings_msg,
        &vec![],
    )
    .unwrap();
    let res_err = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap_err();
    assert_eq!(
        ContractError::CustomError {
            val: "Contract paused".to_string()
        },
        res_err.downcast().unwrap()
    );
    // Set it back
    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &ExecuteMsg::UpdateSettings {
            paused: Some(false),
            owner_id: None,
            // treasury_id: None,
            agent_fee: None,
            min_tasks_per_agent: None,
            agents_eject_threshold: None,
            gas_price: None,
            proxy_callback_gas: None,
            slot_granularity: None,
        },
        &vec![],
    )
    .unwrap();

    // AgentNotRegistered
    let res_err = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap_err();
    assert_eq!(
        ContractError::AgentNotRegistered {},
        res_err.downcast().unwrap()
    );

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();

    // create task, so any slot actually exists
    let res = app
        .execute_contract(
            Addr::unchecked(ANYONE),
            contract_addr.clone(),
            &create_task_msg,
            &coins(315006, NATIVE_DENOM),
        )
        .unwrap();
    // Assert task hash is returned as part of event attributes
    let mut has_created_hash: bool = false;
    for e in res.events {
        for a in e.attributes {
            if a.key == "task_hash" && a.value == task_id_str.clone() {
                has_created_hash = true;
            }
        }
    }
    assert!(has_created_hash);

    // NoTasksForSlot
    let res_no_tasks: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(res_no_tasks, ContractError::NoTaskFound {});

    // NOTE: Unless there's a way to fake a task getting removed but hash remains in slot,
    // this coverage is not mockable. There literally shouldn't be any code that allows
    // this scenario to happen since all slot/task removal cases are covered
    // // delete the task so we test leaving an empty slot
    // app.execute_contract(
    //     Addr::unchecked(ANYONE),
    //     contract_addr.clone(),
    //     &ExecuteMsg::RemoveTask {
    //         task_hash: task_id_str.clone(),
    //     },
    //     &vec![],
    // )
    // .unwrap();

    // // NoTaskFound
    // let res_err = app
    //     .execute_contract(
    //         Addr::unchecked(AGENT0),
    //         contract_addr.clone(),
    //         &proxy_call_msg,
    //         &vec![],
    //     )
    //     .unwrap_err();
    // assert_eq!(
    //     ContractError::NoTaskFound {},
    //     res_err.downcast().unwrap()
    // );

    // TODO: TestCov: Task balance too small

    Ok(())
}

// TODO: TestCov: Agent balance updated (send_base_agent_reward)
// TODO: TestCov: Total balance updated
#[test]
fn proxy_call_success() -> StdResult<()> {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();
    let proxy_call_msg = ExecuteMsg::ProxyCall { task_hash: None };
    let task_id_str =
        "7122ec27799d103d712fff6d1d68ae1e49141fde02926416a2f9ca9f3e98735e".to_string();

    // Doing this msg since its the easiest to guarantee success in reply
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&ExecuteMsg::WithdrawReward {})?,
        funds: coins(1, NATIVE_DENOM),
    });

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: Some(Boundary::Height {
                start: None,
                end: None,
            }),
            stop_on_fail: false,
            actions: vec![Action {
                msg,
                gas_limit: Some(250_000),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };

    // create a task
    let res = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            contract_addr.clone(),
            &create_task_msg,
            &coins(525000, NATIVE_DENOM),
        )
        .unwrap();
    // Assert task hash is returned as part of event attributes
    let mut has_created_hash: bool = false;
    for e in res.events {
        for a in e.attributes {
            if a.key == "task_hash" && a.value == task_id_str.clone() {
                has_created_hash = true;
            }
        }
    }
    assert!(has_created_hash);

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();
    app.execute_contract(
        Addr::unchecked(contract_addr.clone()),
        contract_addr.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // might need block advancement?!
    app.update_block(add_little_time);

    // execute proxy_call
    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap();
    let mut has_required_attributes: bool = true;
    let mut has_submsg_method: bool = false;
    let mut has_reply_success: bool = false;
    let attributes = vec![
        ("method", "proxy_call"),
        ("agent", AGENT0),
        ("slot_id", "12346"),
        ("slot_kind", "Block"),
        ("task_hash", task_id_str.as_str().clone()),
    ];

    // check all attributes are covered in response, and match the expected values
    for (k, v) in attributes.iter() {
        let mut attr_key: Option<String> = None;
        let mut attr_value: Option<String> = None;
        for e in res.clone().events {
            for a in e.attributes {
                if e.ty == "wasm" && a.clone().key == k.to_string() && attr_key.is_none() {
                    attr_key = Some(a.clone().key);
                    attr_value = Some(a.clone().value);
                }
                if e.ty == "wasm"
                    && a.clone().key == "method"
                    && a.clone().value == "withdraw_agent_balance"
                {
                    has_submsg_method = true;
                }
                if e.ty == "reply" && a.clone().key == "mode" && a.clone().value == "handle_success"
                {
                    has_reply_success = true;
                }
            }
        }

        // flip bool if none found, or value doesnt match
        if let Some(_key) = attr_key {
            if let Some(value) = attr_value {
                if v.to_string() != value {
                    has_required_attributes = false;
                }
            } else {
                has_required_attributes = false;
            }
        } else {
            has_required_attributes = false;
        }
    }
    assert!(has_required_attributes);
    assert!(has_submsg_method);
    assert!(has_reply_success);

    Ok(())
}

#[test]
fn proxy_call_no_task_and_withdraw() -> StdResult<()> {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();

    let to_address = String::from("not_you");
    let amount = coin(1000, "atom");
    let send = BankMsg::Send {
        to_address,
        amount: vec![amount],
    };
    let msg: CosmosMsg = send.clone().into();
    let gas_limit = 150_000;

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg,
                gas_limit: Some(gas_limit),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };
    let agent_fee = gas_limit.checked_mul(5).unwrap().checked_div(100).unwrap();
    let amount_for_one_task = gas_limit + agent_fee + 1000;
    // create a task
    let res = app.execute_contract(
        Addr::unchecked(ANYONE),
        contract_addr.clone(),
        &create_task_msg,
        &coins(u128::from(amount_for_one_task * 2), "atom"),
    );
    assert!(res.is_ok());

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();

    app.update_block(add_little_time);

    let res = app.execute_contract(
        Addr::unchecked(AGENT0),
        contract_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    );
    assert!(res.is_ok());

    // Call proxy_call when there is no task, should fail
    let res: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &ExecuteMsg::ProxyCall { task_hash: None },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(res, ContractError::NoTaskFound {});

    let beneficiary_balance_before_proxy_call = app
        .wrap()
        .query_balance(AGENT_BENEFICIARY, NATIVE_DENOM)
        .unwrap();
    // Agent withdraws the reward
    let res = app.execute_contract(
        Addr::unchecked(AGENT0),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawReward {},
        &[],
    );
    assert!(res.is_ok());
    let beneficiary_balance_after_proxy_call = app
        .wrap()
        .query_balance(AGENT_BENEFICIARY, NATIVE_DENOM)
        .unwrap();
    assert_eq!(
        (beneficiary_balance_after_proxy_call.amount
            - beneficiary_balance_before_proxy_call.amount)
            .u128(),
        (agent_fee + gas_limit) as u128
    );

    Ok(())
}

#[test]
fn proxy_callback_fail_cases() -> StdResult<()> {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();
    let proxy_call_msg = ExecuteMsg::ProxyCall { task_hash: None };
    let task_id_str =
        "96003a7938c1ac9566fec1be9b0cfa97a56626a574940ef5968364ef4d30c15a".to_string();

    // Doing this msg since its the easiest to guarantee success in reply
    let validator = String::from("you");
    let amount = coin(3, NATIVE_DENOM);
    let stake = StakingMsg::Delegate { validator, amount };
    let msg: CosmosMsg = stake.clone().into();

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: Some(Boundary::Height {
                start: None,
                end: Some(12347_u64.into()),
            }),
            stop_on_fail: true,
            actions: vec![Action {
                msg,
                gas_limit: Some(250_000),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };

    // create a task
    let res = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            contract_addr.clone(),
            &create_task_msg,
            &coins(525006, NATIVE_DENOM),
        )
        .unwrap();
    // Assert task hash is returned as part of event attributes
    let mut has_created_hash: bool = false;
    for e in res.events {
        for a in e.attributes {
            if a.key == "task_hash" && a.value == task_id_str.clone() {
                has_created_hash = true;
            }
        }
    }
    assert!(has_created_hash);

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();
    app.execute_contract(
        Addr::unchecked(contract_addr.clone()),
        contract_addr.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // might need block advancement?!
    app.update_block(add_little_time);

    // execute proxy_call - STOP ON FAIL
    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap();
    let mut has_required_attributes: bool = true;
    let mut has_submsg_method: bool = false;
    let mut has_reply_success: bool = false;
    let attributes = vec![
        ("method", "remove_task"), // the last method
        ("slot_id", "12346"),
        ("slot_kind", "Block"),
        ("task_hash", task_id_str.as_str().clone()),
    ];

    // check all attributes are covered in response, and match the expected values
    for (k, v) in attributes.iter() {
        let mut attr_key: Option<String> = None;
        let mut attr_value: Option<String> = None;
        for e in res.clone().events {
            for a in e.attributes.clone() {
                if e.ty == "wasm" && a.clone().key == k.to_string() {
                    attr_key = Some(a.clone().key);
                    attr_value = Some(a.clone().value);
                }
                if e.ty == "transfer"
                    && a.clone().key == "amount"
                    && a.clone().value == "525006atom"
                // task didn't pay for the failed execution
                {
                    has_submsg_method = true;
                }
                if e.ty == "reply" && a.clone().key == "mode" && a.clone().value == "handle_failure"
                {
                    has_reply_success = true;
                }
            }
        }

        // flip bool if none found, or value doesnt match
        if let Some(_key) = attr_key {
            if let Some(value) = attr_value {
                if v.to_string() != value {
                    println!("v: {v}, value: {value}");
                    has_required_attributes = false;
                }
            } else {
                has_required_attributes = false;
            }
        } else {
            has_required_attributes = false;
        }
    }
    assert!(has_required_attributes);
    assert!(has_submsg_method);
    assert!(has_reply_success);

    // let task_id_str =
    //     "ce7f88df7816b4cf2d0cd882f189eb81ad66e4a9aabfc1eb5ba2189d73f9929b".to_string();

    // Doing this msg since its the easiest to guarantee success in reply
    let validator = String::from("you");
    let amount = coin(3, NATIVE_DENOM);
    let stake = StakingMsg::Delegate { validator, amount };
    let msg: CosmosMsg = stake.clone().into();

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: Some(Boundary::Height {
                start: None,
                end: Some(12347_u64.into()),
            }),
            stop_on_fail: false,
            actions: vec![Action {
                msg,
                gas_limit: Some(250_000),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };

    // create the task again
    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &create_task_msg,
        &coins(525006, NATIVE_DENOM),
    )
    .unwrap();

    // might need block advancement?!
    app.update_block(add_little_time);
    app.update_block(add_little_time);

    // execute proxy_call - TASK ENDED
    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap();
    let mut has_required_attributes: bool = true;
    let mut has_submsg_method: bool = false;
    let mut has_reply_success: bool = false;
    let attributes = vec![
        ("method", "remove_task"), // the last method
        ("ended_task", task_id_str.as_str().clone()),
    ];

    // check all attributes are covered in response, and match the expected values
    for (k, v) in attributes.iter() {
        let mut attr_key: Option<String> = None;
        let mut attr_value: Option<String> = None;
        for e in res.clone().events {
            for a in e.attributes {
                if e.ty == "wasm" && a.clone().key == k.to_string() {
                    attr_key = Some(a.clone().key);
                    attr_value = Some(a.clone().value);
                }
                if e.ty == "transfer"
                    && a.clone().key == "amount"
                    && a.clone().value == "525006atom"
                // task didn't pay for the failed execution
                {
                    println!("value {:?}", a.clone().value);
                    has_submsg_method = true;
                }
                if e.ty == "reply" && a.clone().key == "mode" && a.clone().value == "handle_failure"
                {
                    has_reply_success = true;
                }
            }
        }

        // flip bool if none found, or value doesnt match
        if let Some(_key) = attr_key {
            if let Some(value) = attr_value {
                if v.to_string() != value {
                    has_required_attributes = false;
                }
            } else {
                has_required_attributes = false;
            }
        } else {
            has_required_attributes = false;
        }
    }
    assert!(has_required_attributes);
    assert!(has_submsg_method);
    assert!(has_reply_success);

    Ok(())
}

#[test]
fn proxy_callback_block_slots() -> StdResult<()> {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();
    let proxy_call_msg = ExecuteMsg::ProxyCall { task_hash: None };
    let task_id_str =
        "7122ec27799d103d712fff6d1d68ae1e49141fde02926416a2f9ca9f3e98735e".to_string();

    // Doing this msg since its the easiest to guarantee success in reply
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&ExecuteMsg::WithdrawReward {})?,
        funds: coins(1, NATIVE_DENOM),
    });

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg,
                gas_limit: Some(250_000),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };

    // create a task
    let res = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            contract_addr.clone(),
            &create_task_msg,
            &coins(525000, NATIVE_DENOM),
        )
        .unwrap();
    // Assert task hash is returned as part of event attributes
    let mut has_created_hash: bool = false;
    for e in res.events {
        for a in e.attributes {
            if a.key == "task_hash" && a.value == task_id_str.clone() {
                has_created_hash = true;
            }
        }
    }
    assert!(has_created_hash);

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();
    app.execute_contract(
        Addr::unchecked(contract_addr.clone()),
        contract_addr.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // might need block advancement?!
    app.update_block(add_little_time);

    // execute proxy_call
    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap();
    let mut has_required_attributes: bool = true;
    let mut has_submsg_method: bool = false;
    let mut has_reply_success: bool = false;
    let attributes = vec![
        ("method", "proxy_callback"),
        ("slot_id", "12347"),
        ("slot_kind", "Block"),
        ("task_hash", task_id_str.as_str().clone()),
    ];

    // check all attributes are covered in response, and match the expected values
    for (k, v) in attributes.iter() {
        let mut attr_key: Option<String> = None;
        let mut attr_value: Option<String> = None;
        for e in res.clone().events {
            for a in e.attributes {
                if e.ty == "wasm" && a.clone().key == k.to_string() {
                    attr_key = Some(a.clone().key);
                    attr_value = Some(a.clone().value);
                }
                if e.ty == "wasm"
                    && a.clone().key == "method"
                    && a.clone().value == "withdraw_agent_balance"
                {
                    has_submsg_method = true;
                }
                if e.ty == "reply" && a.clone().key == "mode" && a.clone().value == "handle_success"
                {
                    has_reply_success = true;
                }
            }
        }

        // flip bool if none found, or value doesnt match
        if let Some(_key) = attr_key {
            if let Some(value) = attr_value {
                if v.to_string() != value {
                    has_required_attributes = false;
                }
            } else {
                has_required_attributes = false;
            }
        } else {
            has_required_attributes = false;
        }
    }
    assert!(has_required_attributes);
    assert!(has_submsg_method);
    assert!(has_reply_success);

    Ok(())
}

#[test]
fn proxy_callback_time_slots() -> StdResult<()> {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();
    let proxy_call_msg = ExecuteMsg::ProxyCall { task_hash: None };
    let task_id_str =
        "29d22d2229b1388da3cf71ff0528c347561e11ee06877a983519eeb34fd67abb".to_string();

    // Doing this msg since its the easiest to guarantee success in reply
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&ExecuteMsg::WithdrawReward {})?,
        funds: coins(1, NATIVE_DENOM),
    });

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Cron("0 * * * * *".to_string()),
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg,
                gas_limit: Some(250_000),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };

    // create a task
    let res = app
        .execute_contract(
            Addr::unchecked(ADMIN),
            contract_addr.clone(),
            &create_task_msg,
            &coins(525000, NATIVE_DENOM),
        )
        .unwrap();
    // Assert task hash is returned as part of event attributes
    let mut has_created_hash: bool = false;
    for e in res.events {
        for a in e.attributes {
            if a.key == "task_hash" && a.value == task_id_str.clone() {
                has_created_hash = true;
            }
        }
    }
    assert!(has_created_hash);

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();
    app.execute_contract(
        Addr::unchecked(contract_addr.clone()),
        contract_addr.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // might need block advancement?!
    app.update_block(add_one_duration_of_time);

    // execute proxy_call
    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap();
    let mut has_required_attributes: bool = true;
    let mut has_submsg_method: bool = false;
    let mut has_reply_success: bool = false;
    let attributes = vec![
        ("method", "proxy_callback"),
        ("slot_id", "1571797860000000000"),
        ("slot_kind", "Cron"),
        ("task_hash", task_id_str.as_str().clone()),
    ];

    // check all attributes are covered in response, and match the expected values
    for (k, v) in attributes.iter() {
        let mut attr_key: Option<String> = None;
        let mut attr_value: Option<String> = None;
        for e in res.clone().events {
            for a in e.attributes {
                if e.ty == "wasm" && a.clone().key == k.to_string() {
                    attr_key = Some(a.clone().key);
                    attr_value = Some(a.clone().value);
                }
                if e.ty == "wasm"
                    && a.clone().key == "method"
                    && a.clone().value == "withdraw_agent_balance"
                {
                    has_submsg_method = true;
                }
                if e.ty == "reply" && a.clone().key == "mode" && a.clone().value == "handle_success"
                {
                    has_reply_success = true;
                }
            }
        }

        // flip bool if none found, or value doesnt match
        if let Some(_key) = attr_key {
            if let Some(value) = attr_value {
                if v.to_string() != value {
                    has_required_attributes = false;
                }
            } else {
                has_required_attributes = false;
            }
        } else {
            has_required_attributes = false;
        }
    }
    assert!(has_required_attributes);
    assert!(has_submsg_method);
    assert!(has_reply_success);

    Ok(())
}

#[test]
fn proxy_call_several_tasks() -> StdResult<()> {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();
    let proxy_call_msg = ExecuteMsg::ProxyCall { task_hash: None };

    // Doing this msg since its the easiest to guarantee success in reply
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&ExecuteMsg::WithdrawReward {})?,
        funds: coins(1, NATIVE_DENOM),
    });

    let msg2 = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&ExecuteMsg::WithdrawReward {})?,
        funds: coins(2, NATIVE_DENOM),
    });

    let msg3 = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&ExecuteMsg::WithdrawReward {})?,
        funds: coins(3, NATIVE_DENOM),
    });

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg,
                gas_limit: Some(250_000),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };

    let create_task_msg2 = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg: msg2,
                gas_limit: Some(250_000),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };

    let create_task_msg3 = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg: msg3,
                gas_limit: Some(250_000),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };

    // create two tasks in the same block
    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &create_task_msg,
        &coins(525000, NATIVE_DENOM),
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &create_task_msg2,
        &coins(525000, NATIVE_DENOM),
    )
    .unwrap();

    // the third task is created in another block
    app.update_block(add_little_time);

    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &create_task_msg3,
        &coins(525000, NATIVE_DENOM),
    )
    .unwrap();

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();
    app.execute_contract(
        Addr::unchecked(contract_addr.clone()),
        contract_addr.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // need block advancement
    app.update_block(add_little_time);

    // execute proxy_call's
    let res = app.execute_contract(
        Addr::unchecked(AGENT0),
        contract_addr.clone(),
        &proxy_call_msg,
        &vec![],
    );
    assert!(res.is_ok());

    let res = app.execute_contract(
        Addr::unchecked(AGENT0),
        contract_addr.clone(),
        &proxy_call_msg,
        &vec![],
    );
    assert!(res.is_ok());

    let res = app.execute_contract(
        Addr::unchecked(AGENT0),
        contract_addr.clone(),
        &proxy_call_msg,
        &vec![],
    );
    assert!(res.is_ok());
    Ok(())
}

#[test]
fn test_proxy_call_with_bank_message() -> StdResult<()> {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();

    let to_address = String::from("not_you");
    let amount = coin(1000, NATIVE_DENOM);
    let send = BankMsg::Send {
        to_address,
        amount: vec![amount],
    };
    let msg: CosmosMsg = send.clone().into();
    let gas_limit = 150_000;

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg,
                gas_limit: Some(gas_limit),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };
    let amount_for_one_task =
        gas_limit + gas_limit.checked_mul(5).unwrap().checked_div(100).unwrap() + 1000;
    // create a task
    let res = app.execute_contract(
        Addr::unchecked(ANYONE),
        contract_addr.clone(),
        &create_task_msg,
        &coins(u128::from(amount_for_one_task * 2), NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();

    app.update_block(add_little_time);

    let res = app.execute_contract(
        Addr::unchecked(AGENT0),
        contract_addr.clone(),
        &ExecuteMsg::ProxyCall { task_hash: None },
        &[],
    );

    assert!(res.is_ok());
    Ok(())
}
#[test]
fn test_proxy_call_with_bank_message_should_fail() -> StdResult<()> {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();

    let to_address = String::from("not_you");
    let amount = coin(600_000, NATIVE_DENOM);
    let send = BankMsg::Send {
        to_address,
        amount: vec![amount],
    };
    let msg: CosmosMsg = send.clone().into();
    let gas_limit: u64 = 150_000;
    let agent_fee = gas_limit.checked_mul(5).unwrap().checked_div(100).unwrap();

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg,
                gas_limit: Some(gas_limit),
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };
    // create 1 token off task
    let amount_for_one_task = gas_limit + agent_fee;
    // create a task
    let res = app.execute_contract(
        Addr::unchecked(ANYONE),
        contract_addr.clone(),
        &create_task_msg,
        &coins(u128::from(amount_for_one_task * 2), NATIVE_DENOM),
    );
    assert!(res.is_err()); //Will fail, abount of send > then task.total_deposit

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();

    app.update_block(add_little_time);

    let res: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &ExecuteMsg::ProxyCall { task_hash: None },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(res, ContractError::NoTaskFound {});

    Ok(())
}

#[test]
fn test_multi_action() {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();

    let addr1 = String::from("addr1");
    let addr2 = String::from("addr2");
    let amount = coins(3, NATIVE_DENOM);
    let send = BankMsg::Send {
        to_address: addr1,
        amount,
    };
    let msg1: CosmosMsg = send.into();
    let amount = coins(4, NATIVE_DENOM);
    let send = BankMsg::Send {
        to_address: addr2,
        amount,
    };
    let msg2: CosmosMsg = send.into();

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Once,
            boundary: None,
            stop_on_fail: false,
            actions: vec![
                Action {
                    msg: msg1,
                    gas_limit: None,
                },
                Action {
                    msg: msg2,
                    gas_limit: None,
                },
            ],
            rules: None,
            cw20_coins: vec![],
        },
    };
    let gas_limit = GAS_BASE_FEE_JUNO;
    let agent_fee = gas_limit.checked_mul(5).unwrap().checked_div(100).unwrap();
    let amount_for_one_task = (gas_limit * 2) + agent_fee * 2 + 3 + 4; // + 3 + 4 atoms sent

    // create a task
    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &create_task_msg,
        &coins(u128::from(amount_for_one_task), NATIVE_DENOM),
    )
    .unwrap();

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();

    app.update_block(add_little_time);

    let proxy_call_msg = ExecuteMsg::ProxyCall { task_hash: None };
    let res = app.execute_contract(
        Addr::unchecked(AGENT0),
        contract_addr.clone(),
        &proxy_call_msg,
        &[],
    );
    assert!(res.is_ok());
}

#[test]
fn test_balance_changes() {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();

    let addr1 = String::from("addr1");
    let addr2 = String::from("addr2");
    let amount = coins(3, NATIVE_DENOM);
    let send = BankMsg::Send {
        to_address: addr1,
        amount,
    };
    let msg1: CosmosMsg = send.into();
    let amount = coins(4, NATIVE_DENOM);
    let send = BankMsg::Send {
        to_address: addr2,
        amount,
    };
    let msg2: CosmosMsg = send.into();

    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Once,
            boundary: None,
            stop_on_fail: false,
            actions: vec![
                Action {
                    msg: msg1,
                    gas_limit: None,
                },
                Action {
                    msg: msg2,
                    gas_limit: None,
                },
            ],
            rules: None,
            cw20_coins: vec![],
        },
    };
    let gas_limit = GAS_BASE_FEE_JUNO;
    let agent_fee = gas_limit.checked_mul(5).unwrap().checked_div(100).unwrap();
    let extra = 50; // extra for checking refunds at task removal
    let amount_for_one_task = (gas_limit * 2) + agent_fee * 2 + 3 + 4 + extra; // + 3 + 4 atoms sent

    // create a task
    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &create_task_msg,
        &coins(u128::from(amount_for_one_task), NATIVE_DENOM),
    )
    .unwrap();

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();

    app.update_block(add_little_time);

    // checking changes to contract balances and to the task creator
    let contract_balance_before_proxy_call = app
        .wrap()
        .query_balance(&contract_addr, NATIVE_DENOM)
        .unwrap();
    let admin_balance_before_proxy_call = app.wrap().query_balance(ADMIN, NATIVE_DENOM).unwrap();
    let proxy_call_msg = ExecuteMsg::ProxyCall { task_hash: None };
    app.execute_contract(
        Addr::unchecked(AGENT0),
        contract_addr.clone(),
        &proxy_call_msg,
        &vec![],
    )
    .unwrap();
    let contract_balance_after_proxy_call = app
        .wrap()
        .query_balance(&contract_addr, NATIVE_DENOM)
        .unwrap();
    assert_eq!(
        contract_balance_after_proxy_call.amount,
        contract_balance_before_proxy_call.amount - Uint128::from(extra + 3 + 4)
    );
    let admin_balance_after_proxy_call = app.wrap().query_balance(ADMIN, NATIVE_DENOM).unwrap();
    assert_eq!(
        admin_balance_after_proxy_call.amount,
        admin_balance_before_proxy_call.amount + Uint128::from(extra)
    );

    // checking balances of recipients
    let balance_addr1 = app.wrap().query_balance("addr1", NATIVE_DENOM).unwrap();
    assert_eq!(
        balance_addr1,
        Coin {
            denom: NATIVE_DENOM.to_string(),
            amount: Uint128::from(3_u128),
        }
    );

    let balance_addr2 = app.wrap().query_balance("addr2", NATIVE_DENOM).unwrap();
    assert_eq!(
        balance_addr2,
        Coin {
            denom: NATIVE_DENOM.to_string(),
            amount: Uint128::from(4_u128),
        }
    );

    // checking balance of agent and contract after withdrawal
    let beneficary_balance_before_withdraw = app
        .wrap()
        .query_balance(AGENT_BENEFICIARY, NATIVE_DENOM)
        .unwrap();
    let contract_balance_before_withdraw = app
        .wrap()
        .query_balance(&contract_addr, NATIVE_DENOM)
        .unwrap();
    let withdraw_msg = ExecuteMsg::WithdrawReward {};
    app.execute_contract(
        Addr::unchecked(AGENT0),
        contract_addr.clone(),
        &withdraw_msg,
        &[],
    )
    .unwrap();
    let beneficary_balance_after_withdraw = app
        .wrap()
        .query_balance(AGENT_BENEFICIARY, NATIVE_DENOM)
        .unwrap();
    let contract_balance_after_withdraw = app
        .wrap()
        .query_balance(&contract_addr, NATIVE_DENOM)
        .unwrap();

    let expected_transfer_amount = Uint128::from(gas_limit * 2 + agent_fee * 2);
    assert_eq!(
        beneficary_balance_after_withdraw.amount,
        beneficary_balance_before_withdraw.amount + expected_transfer_amount
    );
    assert_eq!(
        contract_balance_after_withdraw.amount,
        contract_balance_before_withdraw.amount - expected_transfer_amount
    )
}

#[test]
fn test_no_reschedule_if_lack_balance() {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();

    let addr1 = String::from("addr1");
    let amount = coins(3, NATIVE_DENOM);
    let send = BankMsg::Send {
        to_address: addr1,
        amount,
    };
    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg: send.into(),
                gas_limit: None,
            }],
            rules: None,
            cw20_coins: vec![],
        },
    };

    let gas_limit = GAS_BASE_FEE_JUNO;
    let agent_fee = gas_limit.checked_mul(5).unwrap().checked_div(100).unwrap();
    let extra = 50; // extra for checking nonzero task balance
    let amount_for_one_task = gas_limit + agent_fee + 3; // + 3 atoms sent

    // create a task
    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &create_task_msg,
        &coins(u128::from(amount_for_one_task * 2 + extra - 3), "atom"),
    )
    .unwrap();

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();

    let proxy_call_msg = ExecuteMsg::ProxyCall { task_hash: None };
    // executing it two times
    app.update_block(add_little_time);
    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap();
    assert!(res.events.iter().any(|event| {
        event
            .attributes
            .iter()
            .any(|attr| attr.key == "method" && attr.value == "proxy_callback")
    }));

    let task: Option<TaskResponse> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::GetTask {
                task_hash: "65237042c224447b7d6d7cdfd6515af3e76cb3270ce6d5ed989a6babc12f1026"
                    .to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        task.unwrap().total_deposit[0].amount,
        Uint128::from(GAS_BASE_FEE_JUNO + agent_fee + extra)
    );

    app.update_block(add_little_time);
    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap();
    assert!(res.events.iter().any(|event| {
        event
            .attributes
            .iter()
            .any(|attr| attr.key == "method" && attr.value == "proxy_callback")
    }));
    // third time it pays only base to agent
    // since "extra" is not enough to cover another task and it got removed
    let task: Option<TaskResponse> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::GetTask {
                task_hash: "65237042c224447b7d6d7cdfd6515af3e76cb3270ce6d5ed989a6babc12f1026"
                    .to_string(),
            },
        )
        .unwrap();
    assert!(task.is_none());
    app.update_block(add_little_time);
    let res: ContractError = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &proxy_call_msg,
            &vec![],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(res, ContractError::NoTaskFound {});
}

#[test]
fn test_complete_task_with_rule() {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();
    let task_hash = "259f4b3122822233bee9bc6ec8d38184e4b6ce0908decd68d972639aa92199c7";

    let addr1 = String::from("addr1");
    let amount = coins(3, NATIVE_DENOM);
    let send = BankMsg::Send {
        to_address: addr1,
        amount,
    };
    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Once,
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg: send.clone().into(),
                gas_limit: None,
            }],
            rules: Some(vec![Rule::HasBalanceGte(HasBalanceGte {
                address: String::from("addr2"),
                required_balance: coins(1, NATIVE_DENOM).into(),
            })]),
            cw20_coins: vec![],
        },
    };

    let attached_balance = 900058;
    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &create_task_msg,
        &coins(attached_balance, NATIVE_DENOM),
    )
    .unwrap();

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();

    app.update_block(add_little_time);

    let agent_tasks: Option<AgentTaskResponse> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::GetAgentTasks {
                account_id: String::from(AGENT0),
            },
        )
        .unwrap();
    assert!(agent_tasks.is_none());

    let tasks_with_rules: Vec<TaskWithRulesResponse> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::GetTasksWithRules {
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(tasks_with_rules.len(), 1);
    app.send_tokens(
        Addr::unchecked(ADMIN),
        Addr::unchecked("addr2"),
        &coins(1, NATIVE_DENOM),
    )
    .unwrap();

    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &ExecuteMsg::ProxyCall {
                task_hash: Some(String::from(task_hash)),
            },
            &[],
        )
        .unwrap();

    assert!(res.events.iter().any(|ev| ev
        .attributes
        .iter()
        .any(|attr| attr.key == "task_hash" && attr.value == task_hash)));
    assert!(res.events.iter().any(|ev| ev
        .attributes
        .iter()
        .any(|attr| attr.key == "method" && attr.value == "proxy_callback")));

    let tasks_with_rules: Vec<TaskWithRulesResponse> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::GetTasksWithRules {
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(tasks_with_rules.is_empty());
}

#[test]
fn test_reschedule_task_with_rule() {
    let (mut app, cw_template_contract, _) = proper_instantiate();
    let contract_addr = cw_template_contract.addr();
    let task_hash = "4e74864be3956efe77bafac50944995290a32507bbd4509dd8ff21d3fdfdfec3";

    let addr1 = String::from("addr1");
    let amount = coins(3, NATIVE_DENOM);
    let send = BankMsg::Send {
        to_address: addr1,
        amount,
    };
    let create_task_msg = ExecuteMsg::CreateTask {
        task: TaskRequest {
            interval: Interval::Immediate,
            boundary: None,
            stop_on_fail: false,
            actions: vec![Action {
                msg: send.clone().into(),
                gas_limit: None,
            }],
            rules: Some(vec![Rule::HasBalanceGte(HasBalanceGte {
                address: String::from("addr2"),
                required_balance: coins(1, NATIVE_DENOM).into(),
            })]),
            cw20_coins: vec![],
        },
    };

    let attached_balance = 900058;
    app.execute_contract(
        Addr::unchecked(ADMIN),
        contract_addr.clone(),
        &create_task_msg,
        &coins(attached_balance, NATIVE_DENOM),
    )
    .unwrap();

    // quick agent register
    let msg = ExecuteMsg::RegisterAgent {
        payable_account_id: Some(AGENT_BENEFICIARY.to_string()),
    };
    app.execute_contract(Addr::unchecked(AGENT0), contract_addr.clone(), &msg, &[])
        .unwrap();

    app.update_block(add_little_time);

    let agent_tasks: Option<AgentTaskResponse> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::GetAgentTasks {
                account_id: String::from(AGENT0),
            },
        )
        .unwrap();
    assert!(agent_tasks.is_none());

    let tasks_with_rules: Vec<TaskWithRulesResponse> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::GetTasksWithRules {
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(tasks_with_rules.len(), 1);

    app.send_tokens(
        Addr::unchecked(ADMIN),
        Addr::unchecked("addr2"),
        &coins(1, NATIVE_DENOM),
    )
    .unwrap();

    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &ExecuteMsg::ProxyCall {
                task_hash: Some(String::from(task_hash)),
            },
            &[],
        )
        .unwrap();

    assert!(res.events.iter().any(|ev| ev
        .attributes
        .iter()
        .any(|attr| attr.key == "task_hash" && attr.value == task_hash)));
    assert!(res.events.iter().any(|ev| ev
        .attributes
        .iter()
        .any(|attr| attr.key == "method" && attr.value == "proxy_callback")));

    let tasks_with_rules: Vec<TaskWithRulesResponse> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::GetTasksWithRules {
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(tasks_with_rules.len(), 1);

    // Shouldn't affect tasks without rules
    let tasks_response: Vec<TaskResponse> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::GetTasks {
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(tasks_response.is_empty());
    let res = app
        .execute_contract(
            Addr::unchecked(AGENT0),
            contract_addr.clone(),
            &ExecuteMsg::ProxyCall {
                task_hash: Some(String::from(task_hash)),
            },
            &[],
        )
        .unwrap();
    assert!(res.events.iter().any(|ev| ev
        .attributes
        .iter()
        .any(|attr| attr.key == "task_hash" && attr.value == task_hash)));
    assert!(res.events.iter().any(|ev| ev
        .attributes
        .iter()
        .any(|attr| attr.key == "method" && attr.value == "proxy_callback")));
    let tasks_with_rules: Vec<TaskWithRulesResponse> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::GetTasksWithRules {
                from_index: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(tasks_with_rules.is_empty());
}