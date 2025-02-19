use std::vec;

use cosmwasm_std::{
    coin, to_binary, Addr, BankMsg, Binary, BlockInfo, Coin, CosmosMsg, Deps, DepsMut, Empty,
    MessageInfo, QuerierWrapper, Reply, Response, StdError, StdResult, Storage, SubMsg, Uint128,
    WasmMsg, WasmQuery,
};
use croncat_sdk_agents::msg::AgentResponse;
use croncat_sdk_core::{internal_messages::agents::AgentOnTaskCompleted, types::AmountForOneTask};
use croncat_sdk_manager::types::{Config, TaskBalance};
use croncat_sdk_tasks::types::{Boundary, CosmosQuery, TaskInfo};
use cw20::{Cw20CoinVerified, Cw20ExecuteMsg};
use serde_cw_value::Value;

use crate::{
    balances::{add_fee_rewards, add_user_cw20},
    contract::TASK_REPLY,
    state::{QueueItem, CONFIG, REPLY_QUEUE, TASKS_BALANCES},
    ContractError,
};

/// Check if contract is paused or user attached redundant funds.
/// Called before every method, except [crate::contract::execute_update_config]
pub(crate) fn check_ready_for_execution(
    info: &MessageInfo,
    paused: bool,
) -> Result<(), ContractError> {
    if paused {
        Err(ContractError::ContractPaused {})
    } else if !info.funds.is_empty() {
        Err(ContractError::RedundantFunds {})
    } else {
        Ok(())
    }
}

pub(crate) fn get_tasks_addr(
    deps_queries: &QuerierWrapper<Empty>,
    config: &Config,
) -> Result<Addr, ContractError> {
    let (tasks_name, version) = &config.croncat_tasks_key;
    croncat_sdk_factory::state::CONTRACT_ADDRS
        .query(
            deps_queries,
            config.croncat_factory_addr.clone(),
            (tasks_name, version),
        )?
        .ok_or(ContractError::InvalidKey {})
}

pub(crate) fn check_if_sender_is_tasks(
    deps_queries: &QuerierWrapper<Empty>,
    config: &Config,
    sender: &Addr,
) -> Result<(), ContractError> {
    let tasks_addr = get_tasks_addr(deps_queries, config)?;
    if tasks_addr != *sender {
        return Err(ContractError::Unauthorized {});
    }

    Ok(())
}

pub(crate) fn get_agents_addr(
    deps_queries: &QuerierWrapper<Empty>,
    config: &Config,
) -> Result<Addr, ContractError> {
    let (agents_name, version) = &config.croncat_agents_key;
    croncat_sdk_factory::state::CONTRACT_ADDRS
        .query(
            deps_queries,
            config.croncat_factory_addr.clone(),
            (agents_name, version),
        )?
        .ok_or(ContractError::InvalidKey {})
}

pub(crate) fn gas_with_fees(gas_amount: u64, fee: u64) -> Result<u64, ContractError> {
    gas_fee(gas_amount, fee)?
        .checked_add(gas_amount)
        .ok_or(ContractError::InvalidGasCalculation {})
}

pub(crate) fn gas_fee(gas_amount: u64, fee: u64) -> Result<u64, ContractError> {
    gas_amount
        .checked_mul(fee)
        .and_then(|n| n.checked_div(100))
        .ok_or(ContractError::InvalidGasCalculation {})
}

pub(crate) fn attached_natives(
    native_denom: &str,
    funds: Vec<Coin>,
) -> Result<(Uint128, Option<Coin>), ContractError> {
    let mut token: Option<Coin> = None;
    let mut native = Uint128::zero();
    for f in funds {
        if f.denom == native_denom {
            native += f.amount;
        } else {
            token = Some(f);
        }
    }
    Ok((native, token))
}

pub(crate) fn calculate_required_natives(
    amount_for_one_task_coins: [Option<Coin>; 2],
    native_denom: &str,
) -> Result<(Uint128, Option<Coin>), ContractError> {
    let res = match amount_for_one_task_coins {
        [Some(c1), Some(c2)] => {
            if c1.denom == native_denom {
                (c1.amount, Some(c2))
            } else if c2.denom == native_denom {
                (c2.amount, Some(c1))
            } else {
                return Err(ContractError::InvalidAttachedCoins {});
            }
        }
        [Some(c1), None] => {
            if c1.denom == native_denom {
                (c1.amount, None)
            } else {
                (Uint128::zero(), Some(c1))
            }
        }
        [None, None] => (Uint128::zero(), None),
        [None, Some(_)] => unreachable!(),
    };
    Ok(res)
}
pub(crate) fn assert_caller_is_agent_contract(
    deps_queries: &QuerierWrapper<Empty>,
    config: &Config,
    sender: &Addr,
) -> Result<(), ContractError> {
    let addr = get_agents_addr(deps_queries, config)?;
    if addr != *sender {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

pub fn query_agent(
    querier: &QuerierWrapper<Empty>,
    config: &Config,
    agent_id: String,
) -> Result<AgentResponse, ContractError> {
    let addr = get_agents_addr(querier, config)?;

    // Get the agent from the agent contract
    let response: AgentResponse = querier.query_wasm_smart(
        addr,
        &croncat_sdk_agents::msg::QueryMsg::GetAgent {
            account_id: agent_id,
        },
    )?;

    Ok(response)
}

pub(crate) fn create_bank_send_message(
    to: &Addr,
    denom: &str,
    amount: u128,
) -> StdResult<CosmosMsg> {
    let coin = coin(amount, denom.to_owned());
    let msg = BankMsg::Send {
        to_address: to.into(),
        amount: vec![coin],
    };

    Ok(msg.into())
}

/// Get sub messages for this task
/// To minimize gas consumption for loads we only reply on failure
/// And the last item to calculate rewards and reschedule or removal of the task
pub(crate) fn task_sub_msgs(task: &croncat_sdk_tasks::types::TaskInfo) -> Vec<SubMsg> {
    let mut sub_msgs = Vec::with_capacity(task.actions.len());
    let mut actions_iter = task.actions.iter().enumerate();

    // safe unwrap here, we don't allow empty actions
    let (last_idx, last_action) = actions_iter.next_back().unwrap();

    for (idx, action) in actions_iter {
        if let Some(gas_limit) = action.gas_limit {
            sub_msgs.push(
                SubMsg::reply_on_error(action.msg.clone(), idx as u64).with_gas_limit(gas_limit),
            );
        } else {
            sub_msgs.push(SubMsg::reply_on_error(action.msg.clone(), idx as u64));
        }
    }
    if let Some(gas_limit) = last_action.gas_limit {
        sub_msgs.push(
            SubMsg::reply_always(last_action.msg.clone(), last_idx as u64)
                .with_gas_limit(gas_limit),
        );
    } else {
        sub_msgs.push(SubMsg::reply_always(
            last_action.msg.clone(),
            last_idx as u64,
        ));
    }
    sub_msgs
}

pub(crate) fn parse_reply_msg(
    storage: &mut dyn Storage,
    queue_item: &mut QueueItem,
    msg: Reply,
) -> bool {
    let id = msg.id as usize;
    if let cosmwasm_std::SubMsgResult::Err(err) = msg.result {
        queue_item.failures.push((id as u8, err));
    }
    let last = queue_item.task.actions.len() == id + 1;
    // If last action let's clean state here
    if last {
        REPLY_QUEUE.remove(storage)
    }
    last
}

pub(crate) fn finalize_task(
    deps: DepsMut,
    queue_item: QueueItem,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut task_balance =
        TASKS_BALANCES.load(deps.storage, queue_item.task.task_hash.as_bytes())?;
    // Sub native for gas
    let gas_with_fees = gas_with_fees(
        queue_item.task.amount_for_one_task.gas,
        (queue_item.task.amount_for_one_task.agent_fee
            + queue_item.task.amount_for_one_task.treasury_fee) as u64,
    )?;
    let native_for_gas_required = queue_item
        .task
        .amount_for_one_task
        .gas_price
        .calculate(gas_with_fees)
        .unwrap();
    task_balance.native_balance = task_balance
        .native_balance
        .checked_sub(Uint128::new(native_for_gas_required))
        .map_err(StdError::overflow)?;

    add_fee_rewards(
        deps.storage,
        queue_item.task.amount_for_one_task.gas,
        &queue_item.task.amount_for_one_task.gas_price,
        &queue_item.agent_addr,
        queue_item.task.amount_for_one_task.agent_fee,
        queue_item.task.amount_for_one_task.treasury_fee,
        false,
    )?;

    let original_amounts = queue_item.task.amount_for_one_task.clone();
    let amounts_without_failed_txs = amounts_without_failed_txs(&queue_item)?;

    // Sub transferred coins
    for coin in amounts_without_failed_txs.coin.iter().flatten() {
        task_balance.sub_coin(coin, &config.native_denom)?;
    }
    // Sub transferred cw20s
    if let Some(cw20) = &amounts_without_failed_txs.cw20 {
        task_balance.sub_cw20(cw20)?;
    }
    let (native_for_sends_required, ibc_required) =
        calculate_required_natives(original_amounts.coin, &config.native_denom)?;

    // unregister task and return unused deposits if any of this:
    // - not recurring
    // - should stop on fail
    // - task balance drained
    if matches!(
        queue_item.task.interval,
        croncat_sdk_tasks::types::Interval::Once
    ) || (queue_item.task.stop_on_fail && !queue_item.failures.is_empty())
        || task_balance
            .verify_enough_attached(
                native_for_sends_required + Uint128::new(native_for_gas_required),
                original_amounts.cw20,
                ibc_required,
                false,
                &config.native_denom,
            )
            .is_err()
    {
        // Transfer unused balances to the task creator and cw20s to the temp balances
        let task_hash = queue_item.task.task_hash;
        let coins_transfer = remove_task_balance(
            deps.storage,
            task_balance,
            &queue_item.task.owner_addr,
            &config.native_denom,
            task_hash.as_bytes(),
        )?;
        // Remove task on tasks contract
        let tasks_addr = get_tasks_addr(&deps.querier, &config)?;
        let msg = croncat_sdk_core::internal_messages::tasks::TasksRemoveTaskByManager {
            task_hash: task_hash.clone().into_bytes(),
        }
        .into_cosmos_msg(tasks_addr)?;
        Ok(Response::new()
            .add_message(msg)
            .add_message(BankMsg::Send {
                to_address: queue_item.task.owner_addr.into_string(),
                amount: coins_transfer,
            })
            .add_attribute("lifecycle", "task_ended")
            .add_attribute("task_hash", task_hash))
    } else {
        let tasks_addr = get_tasks_addr(&deps.querier, &config)?;
        TASKS_BALANCES.save(
            deps.storage,
            queue_item.task.task_hash.as_bytes(),
            &task_balance,
        )?;
        let msg = croncat_sdk_core::internal_messages::tasks::TasksRescheduleTask {
            task_hash: queue_item.task.task_hash.into_bytes(),
        }
        .into_cosmos_msg(tasks_addr)?;
        Ok(Response::new().add_submessage(SubMsg::reply_always(msg, TASK_REPLY)))
    }
}

pub(crate) fn amounts_without_failed_txs(queue_item: &QueueItem) -> StdResult<AmountForOneTask> {
    let mut amounts = queue_item.task.amount_for_one_task.clone();
    for (idx, _) in queue_item.failures.iter() {
        match &queue_item.task.actions[(*idx) as usize].msg {
            CosmosMsg::Bank(BankMsg::Send { amount, .. }) => {
                for coin in amount {
                    amounts.sub_coin(coin)?;
                }
            }
            CosmosMsg::Wasm(WasmMsg::Execute {
                msg, contract_addr, ..
            }) => {
                if let Ok(cw20_msg) = cosmwasm_std::from_binary(msg) {
                    match cw20_msg {
                        Cw20ExecuteMsg::Send { amount, .. } => {
                            amounts.sub_cw20(&Cw20CoinVerified {
                                // Addr safe here because we checked it at `is_valid_msg_calculate_usage`
                                address: Addr::unchecked(contract_addr),
                                amount,
                            })?;
                        }
                        Cw20ExecuteMsg::Transfer { amount, .. } => {
                            amounts.sub_cw20(&Cw20CoinVerified {
                                address: Addr::unchecked(contract_addr),
                                amount,
                            })?;
                        }
                        _ => (),
                    };
                }
            }
            _ => (),
        }
    }
    Ok(amounts)
}

/// This function will
/// - Consume `TaskBalance`
/// - Move unused cw20's to the temp balances
/// - Return any unused coins for the use in the message
pub(crate) fn remove_task_balance(
    storage: &mut dyn Storage,
    task_balance: TaskBalance,
    task_owner: &Addr,
    native_denom: &str,
    task_hash: &[u8],
) -> StdResult<Vec<Coin>> {
    let mut coins_transfer = vec![];
    if task_balance.native_balance > Uint128::zero() {
        coins_transfer.push(coin(task_balance.native_balance.u128(), native_denom))
    }

    if let Some(ibc) = task_balance.ibc_balance {
        if ibc.amount > Uint128::zero() {
            coins_transfer.push(ibc);
        }
    }

    if let Some(cw20) = task_balance.cw20_balance {
        // Back to the temp balance
        add_user_cw20(storage, task_owner, &cw20)?;
    }
    TASKS_BALANCES.remove(storage, task_hash);
    Ok(coins_transfer)
}

/// Check for calls of our contracts
pub(crate) fn check_for_self_calls(
    tasks_addr: &Addr,
    manager_addr: &Addr,
    agents_addr: &Addr,
    manager_owner_addr: &Addr,
    sender: &Addr,
    contract_addr: &String,
    msg: &Binary,
) -> Result<(), ContractError> {
    // If it one of the our contracts it should be a manager
    if contract_addr == tasks_addr || contract_addr == agents_addr {
        return Err(ContractError::UnauthorizedMethod {});
    } else if contract_addr == manager_addr {
        // Check if caller is manager owner
        if sender != manager_owner_addr {
            return Err(ContractError::UnauthorizedMethod {});
        } else if let Ok(msg) = cosmwasm_std::from_binary(msg) {
            // Check if it's tick
            if !matches!(msg, croncat_sdk_agents::msg::ExecuteMsg::Tick {}) {
                return Err(ContractError::UnauthorizedMethod {});
            }
            // Other messages not allowed
        } else {
            return Err(ContractError::UnauthorizedMethod {});
        }
    }
    Ok(())
}

// Check if we're before boundary start
pub(crate) fn is_before_boundary(block_info: &BlockInfo, boundary: Option<&Boundary>) -> bool {
    match boundary {
        Some(Boundary::Time(boundary_time)) => boundary_time
            .start
            .map_or(false, |s| s.nanos() > block_info.time.nanos()),
        Some(Boundary::Height(boundary_height)) => boundary_height
            .start
            .map_or(false, |s| s.u64() > block_info.height),
        None => false,
    }
}

// Check if we're after boundary end
pub(crate) fn is_after_boundary(block_info: &BlockInfo, boundary: Option<&Boundary>) -> bool {
    match boundary {
        Some(Boundary::Time(boundary_time)) => boundary_time
            .end
            .map_or(false, |e| e.nanos() <= block_info.time.nanos()),
        Some(Boundary::Height(boundary_height)) => boundary_height
            .end
            .map_or(false, |e| e.u64() < block_info.height - 1),
        None => false,
    }
}

/// Query all task queries in sequence, return all binary data if found.
/// Response order is important to maintain for transforms
/// NOTE: SystemError's such as InvalidRequest or UnsupportedRequest are not match-able in the QuerierWrapper
/// As such, we require the task creator to validate queries externally before creating a task
/// either with simulation or direct checks. Future support possible but not created here.
///
/// NOTE: Future impls will include recursive query transforms, hence the task info here
pub fn process_queries(
    deps: &DepsMut,
    task: &TaskInfo,
) -> Result<Vec<Option<cosmwasm_std::Binary>>, ContractError> {
    let mut responses: Vec<Option<Binary>> =
        Vec::with_capacity(task.queries.as_ref().unwrap().len());

    let queries = if let Some(qs) = &task.queries {
        qs
    } else {
        // No error here since we want to just simply progress through call stack if we dont have queries
        // Likely this is NOOP
        return Ok(vec![]);
    };

    // Process all the queries
    for query in queries {
        match query {
            CosmosQuery::Croncat(q) => {
                let res: mod_sdk::types::QueryResponse = deps.querier.query(
                    &WasmQuery::Smart {
                        contract_addr: q.contract_addr.clone(),
                        msg: q.msg.clone(),
                    }
                    .into(),
                )?;
                if q.check_result && !res.result {
                    return Err(ContractError::TaskQueryResultFalse {});
                }
                responses.push(Some(res.data));
            }
            CosmosQuery::Wasm(wq) => {
                // Cover all native wasm query types
                match wq {
                    WasmQuery::Smart { contract_addr, msg } => {
                        let data: Result<Value, StdError> = deps.querier.query(
                            &WasmQuery::Smart {
                                contract_addr: contract_addr.clone().to_string(),
                                msg: msg.clone(),
                            }
                            .into(),
                        );
                        match data {
                            Err(..) => responses.push(None),
                            Ok(d) => {
                                // Chuck it in there already!
                                responses.push(Some(to_binary(&d)?));
                            }
                        }
                    }
                    WasmQuery::Raw { contract_addr, key } => {
                        let res: Result<Option<Vec<u8>>, StdError> = deps
                            .querier
                            .query_wasm_raw(contract_addr.clone().to_string(), key.clone());
                        match res {
                            Err(..) => responses.push(None),
                            Ok(d) => {
                                if let Some(r) = d {
                                    responses.push(Some(to_binary(&r)?));
                                } else {
                                    responses.push(None)
                                }
                            }
                        }
                    }
                    WasmQuery::ContractInfo { contract_addr } => {
                        let res = deps
                            .querier
                            .query_wasm_contract_info(contract_addr.clone().to_string());
                        match res {
                            Err(..) => responses.push(None),
                            Ok(d) => {
                                // lets find out whos responsible for this code already!
                                responses.push(Some(to_binary(&d)?));
                            }
                        }
                    }
                    // // NOTE: This is dependent on features = ["cosmwasm_1_2"]
                    // WasmQuery::CodeInfo { code_id } => {
                    //     let res = deps.querier.query_wasm_code_info(*code_id);
                    //     match res {
                    //         Err(..) => responses.push(None),
                    //         Ok(d) => {
                    //             // super helpful for security checks against checksum or code_id changes bruv
                    //             responses.push(Some(to_binary(&d)?));
                    //         }
                    //     }
                    // }
                    _ => {
                        return Err(ContractError::Std(StdError::GenericErr {
                            msg: "Unknown Query Type".to_string(),
                        }));
                    }
                }
            }
        }
    }

    Ok(responses)
}

/// Replace action values with the result value from the queries
/// As long as the transforms are valid
///
/// Gotchas:
/// 1. Transforms will only validate indexes, but never content
/// 2. Transforms are strict: If a query cannot find data or replace as intended, error
/// 3. Only supported message types, otherwise dont use a transform until supported
pub fn replace_values(
    task: &mut TaskInfo,
    query_response_data: Vec<Option<cosmwasm_std::Binary>>,
) -> Result<(), ContractError> {
    for transform in task.transforms.iter() {
        // Validate transform index range
        if transform.action_idx as usize > task.actions.len() - 1 {
            return Err(ContractError::TaskInvalidTransform {});
        }
        match &task.queries {
            Some(q) => {
                if transform.query_idx as usize > q.len() - 1 {
                    return Err(ContractError::TaskInvalidTransform {});
                }
            }
            None => return Err(ContractError::TaskInvalidTransform {}),
        }

        // Process known queries
        if let Some(query_bin) = query_response_data
            .get(transform.query_idx as usize)
            .and_then(|opt| opt.as_ref())
        {
            let mut q_val = cosmwasm_std::from_binary(query_bin)
                .map_err(|e| StdError::generic_err(e.to_string()))?;
            let replace_value = transform.query_response_path.find_value(&mut q_val)?;

            if let Some(action) = task.actions.get_mut(transform.action_idx as usize) {
                // NOTE: This only covers the supported methods known to valid task actions!
                match &mut action.msg {
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: _,
                        msg,
                        funds: _,
                    }) => {
                        let mut action_value = cosmwasm_std::from_binary(msg)
                            .map_err(|e| StdError::generic_err(e.to_string()))?;
                        let replaced_value = transform.action_path.find_value(&mut action_value)?;
                        *replaced_value = replace_value.clone();
                        *msg = Binary(
                            serde_json_wasm::to_vec(&action_value)
                                .map_err(|e| StdError::generic_err(e.to_string()))?,
                        );
                    }
                    CosmosMsg::Bank(BankMsg::Send { to_address, amount }) => {
                        let mut action_value = serde_json_wasm::from_str::<Value>(&format!(
                            r#"{{"bank":{{"send":{{"to_address": "{}", "amount": {}}}}}}}"#,
                            to_address,
                            serde_json_wasm::to_string(amount).unwrap()
                        ))
                        .unwrap();
                        let replaced_value = transform.action_path.find_value(&mut action_value)?;
                        *replaced_value = replace_value.clone();

                        // we mutated, now apply back to bank msg
                        // could this be more insane? probably. For now, hooray we hax'd it to werkz! :D
                        if let Value::Map(parent_map) = &action_value {
                            if let Some(Value::Map(bank_send_map)) =
                                &parent_map.get(&Value::String("bank".to_string()))
                            {
                                if let Some(Value::Map(ref send_map)) =
                                    bank_send_map.get(&Value::String("send".to_string()))
                                {
                                    if let Some(Value::String(ref new_to_address)) =
                                        send_map.get(&Value::String("to_address".to_string()))
                                    {
                                        *to_address = new_to_address.clone();
                                    }
                                    if let Some(Value::Seq(ref new_amount)) =
                                        send_map.get(&Value::String("amount".to_string()))
                                    {
                                        let coins: Vec<Coin> = new_amount
                                            .iter()
                                            .filter_map(|value| {
                                                if let Value::Map(coin_map) = value {
                                                    let amount = coin_map
                                                        .get(&Value::String("amount".to_string()))
                                                        .and_then(|v| {
                                                            if let Value::String(amount_str) = v {
                                                                amount_str.parse::<u128>().ok()
                                                            } else {
                                                                None
                                                            }
                                                        });
                                                    let denom = coin_map
                                                        .get(&Value::String("denom".to_string()))
                                                        .and_then(|v| {
                                                            if let Value::String(denom_str) = v {
                                                                Some(denom_str.to_string())
                                                            } else {
                                                                None
                                                            }
                                                        });

                                                    if let (Some(amount), Some(denom)) =
                                                        (amount, denom)
                                                    {
                                                        Some(Coin {
                                                            denom,
                                                            amount: amount.into(),
                                                        })
                                                    } else {
                                                        None
                                                    }
                                                } else {
                                                    None
                                                }
                                            })
                                            .collect();
                                        *amount = coins;
                                    }
                                }
                            }
                        }
                    }
                    _ => return Err(ContractError::TaskTransformUnsupported {}),
                }
            }
        }
    }
    Ok(())
}

/// Recalculate cw20 usage for this task
/// And check for self-calls
/// It can be initially zero, but after transform we still have to check it does have only one type of cw20
/// If it had initially cw20, it can't change cw20 type
pub(crate) fn recalculate_cw20(
    task: &TaskInfo,
    config: &Config,
    deps: Deps,
    manager_addr: &Addr,
) -> Result<Option<Cw20CoinVerified>, ContractError> {
    let mut current_cw20 = task
        .amount_for_one_task
        .cw20
        .as_ref()
        .map(|cw20| cw20.address.clone());
    let mut cw20_amount = Uint128::zero();
    let agents_addr = get_agents_addr(&deps.querier, config)?;
    let tasks_addr = get_tasks_addr(&deps.querier, config)?;
    let actions = task.actions.iter();
    for action in actions {
        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr, msg, ..
        }) = &action.msg
        {
            check_for_self_calls(
                &tasks_addr,
                manager_addr,
                &agents_addr,
                &config.owner_addr,
                &task.owner_addr,
                contract_addr,
                msg,
            )?;
            let validated_addr = deps.api.addr_validate(contract_addr)?;
            if let Ok(cw20_msg) = cosmwasm_std::from_binary::<Cw20ExecuteMsg>(msg) {
                // Don't let change type of cw20
                if let Some(cw20_addr) = &current_cw20 {
                    if validated_addr.ne(cw20_addr) {
                        return Err(ContractError::TaskNoLongerValid {});
                    }
                } else {
                    current_cw20 = Some(validated_addr);
                }
                match cw20_msg {
                    Cw20ExecuteMsg::Send { amount, .. } if !amount.is_zero() => {
                        cw20_amount = cw20_amount
                            .checked_add(amount)
                            .map_err(StdError::overflow)?;
                    }
                    Cw20ExecuteMsg::Transfer { amount, .. } if !amount.is_zero() => {
                        cw20_amount = cw20_amount
                            .checked_add(amount)
                            .map_err(StdError::overflow)?;
                    }
                    _ => {
                        return Err(ContractError::TaskNoLongerValid {});
                    }
                }
            }
        }
    }
    Ok(current_cw20.map(|addr| Cw20CoinVerified {
        address: addr,
        amount: cw20_amount,
    }))
}

pub(crate) fn check_if_sender_is_task_owner(
    querier: &QuerierWrapper,
    tasks_addr: &Addr,
    sender: &Addr,
    task_hash: &str,
) -> Result<(), ContractError> {
    let task_response: croncat_sdk_tasks::types::TaskResponse = querier.query_wasm_smart(
        tasks_addr,
        &croncat_sdk_tasks::msg::TasksQueryMsg::Task {
            task_hash: task_hash.to_owned(),
        },
    )?;
    let Some(task) = task_response.task else {
        return Err(ContractError::NoTaskHash {  });
    };
    if task.owner_addr.ne(sender) {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

pub fn create_task_completed_msg(
    querier: &QuerierWrapper<Empty>,
    config: &Config,
    agent_id: &Addr,
    is_block_slot_task: bool,
) -> Result<CosmosMsg, ContractError> {
    let addr = get_agents_addr(querier, config)?;
    let args = AgentOnTaskCompleted {
        agent_id: agent_id.to_owned(),
        is_block_slot_task,
    };
    let execute = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: addr.into(),
        msg: to_binary(&croncat_sdk_agents::msg::ExecuteMsg::OnTaskCompleted(args))?,
        funds: vec![],
    });

    Ok(execute)
}
