#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Attribute, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo,
    Reply, Response, StdError, StdResult, Uint128, WasmQuery,
};
use croncat_sdk_core::internal_messages::agents::AgentWithdrawOnRemovalArgs;
use croncat_sdk_core::internal_messages::manager::{ManagerCreateTaskBalance, ManagerRemoveTask};

use croncat_sdk_manager::msg::AgentWithdrawCallback;
use croncat_sdk_manager::types::{TaskBalance, TaskBalanceResponse, UpdateConfig};
use croncat_sdk_tasks::types::{Interval, TaskInfo};
use cw2::set_contract_version;
use cw_utils::parse_reply_execute_data;

use crate::balances::{
    add_fee_rewards, execute_owner_withdraw, execute_receive_cw20, execute_refill_native_balance,
    execute_refill_task_cw20, execute_user_withdraw, query_users_balances, sub_user_cw20,
};
use crate::error::ContractError;
use crate::helpers::{
    assert_caller_is_agent_contract, attached_natives, calculate_required_natives,
    check_if_sender_is_tasks, check_ready_for_execution, create_bank_send_message,
    create_task_completed_msg, finalize_task, gas_with_fees, get_agents_addr, get_tasks_addr,
    is_after_boundary, is_before_boundary, parse_reply_msg, queries_transforms_fallback,
    query_agent, recalculate_cw20, remove_task_balance, replace_values, task_sub_msgs,
};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    Config, QueueItem, AGENT_REWARDS, CONFIG, REPLY_QUEUE, TASKS_BALANCES, TREASURY_BALANCE,
};

pub(crate) const CONTRACT_NAME: &str = "crate:croncat-manager";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) const DEFAULT_FEE: u64 = 5;

/// reply id from tasks contract
pub(crate) const TASK_REPLY: u64 = u64::from_be_bytes(*b"croncat1");

/// Instantiate
/// First contract method before it runs on the chains
/// See [`InstantiateMsg`] for more details
/// `gas_price` and `owner_id` getting validated
///
/// Response: every [`Config`] field as attributes
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // Deconstruct so we don't miss fields
    let InstantiateMsg {
        denom,
        version,
        croncat_tasks_key,
        croncat_agents_key,
        owner_addr,
        gas_price,
        treasury_addr,
        cw20_whitelist,
    } = msg;

    let gas_price = gas_price.unwrap_or_default();
    // Make sure gas_price is valid
    if !gas_price.is_valid() {
        return Err(ContractError::InvalidGasPrice {});
    }

    let owner_addr = owner_addr
        .map(|human| deps.api.addr_validate(&human))
        .transpose()?
        .unwrap_or_else(|| info.sender.clone());

    //Check if we attached some funds in native denom, add them into treasury
    let treasury_funds = info.funds.iter().find(|coin| coin.denom == denom);
    if let Some(funds) = treasury_funds {
        TREASURY_BALANCE.save(deps.storage, &funds.amount)?;
    } else {
        TREASURY_BALANCE.save(deps.storage, &Uint128::zero())?;
    }

    let cw20_whitelist = cw20_whitelist
        .unwrap_or_default()
        .into_iter()
        .map(|human| deps.api.addr_validate(&human))
        .collect::<StdResult<_>>()?;

    let config = Config {
        paused: false,
        owner_addr,
        croncat_factory_addr: info.sender,
        croncat_tasks_key,
        croncat_agents_key,
        agent_fee: DEFAULT_FEE,
        treasury_fee: DEFAULT_FEE,
        gas_price,
        cw20_whitelist,
        native_denom: denom,
        limit: 100,
        treasury_addr: treasury_addr
            .map(|human| deps.api.addr_validate(&human))
            .transpose()?,
    };

    // Update state
    CONFIG.save(deps.storage, &config)?;
    set_contract_version(
        deps.storage,
        CONTRACT_NAME,
        version.unwrap_or_else(|| CONTRACT_VERSION.to_string()),
    )?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("paused", config.paused.to_string())
        .add_attribute("owner_id", config.owner_addr.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig(msg) => execute_update_config(deps, info, *msg),
        ExecuteMsg::ProxyCall { task_hash } => execute_proxy_call(deps, env, info, task_hash),
        ExecuteMsg::Receive(msg) => execute_receive_cw20(deps, info, msg),
        ExecuteMsg::RefillTaskBalance { task_hash } => {
            execute_refill_native_balance(deps, info, task_hash)
        }
        ExecuteMsg::RefillTaskCw20Balance { task_hash, cw20 } => {
            execute_refill_task_cw20(deps, info, task_hash, cw20)
        }
        ExecuteMsg::CreateTaskBalance(msg) => execute_create_task_balance(deps, info, msg),
        ExecuteMsg::RemoveTask(msg) => execute_remove_task(deps, info, msg),
        ExecuteMsg::OwnerWithdraw {} => execute_owner_withdraw(deps, info),
        ExecuteMsg::UserWithdraw { limit } => execute_user_withdraw(deps, info, limit),
        ExecuteMsg::AgentWithdraw(args) => execute_withdraw_agent_rewards(deps, info, args),
    }
}

fn execute_remove_task(
    deps: DepsMut,
    info: MessageInfo,
    msg: ManagerRemoveTask,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_if_sender_is_tasks(&deps.querier, &config, &info.sender)?;
    let task_owner = msg.sender;
    let task_balance = TASKS_BALANCES.load(deps.storage, &msg.task_hash)?;
    let coins_transfer = remove_task_balance(
        deps.storage,
        task_balance,
        &task_owner,
        &config.native_denom,
        &msg.task_hash,
    )?;

    let bank_send = BankMsg::Send {
        to_address: task_owner.into_string(),
        amount: coins_transfer,
    };
    Ok(Response::new()
        .add_attribute("action", "remove_task")
        .add_message(bank_send))
}

fn execute_proxy_call(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task_hash: Option<String>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    check_ready_for_execution(&info, &config)?;

    let agent_addr = info.sender;
    let agents_addr = get_agents_addr(&deps.querier, &config)?;
    let tasks_addr = get_tasks_addr(&deps.querier, &config)?;
    let pre_event_based = task_hash.is_some();
    // Check if agent is active,
    // Then get a task
    let current_task: croncat_sdk_tasks::types::TaskResponse = if let Some(hash) = task_hash {
        // For evented case - check the agent is active, then may the best agent win
        let agent_response: croncat_sdk_agents::msg::AgentResponse =
            deps.querier.query_wasm_smart(
                agents_addr,
                &croncat_sdk_agents::msg::QueryMsg::GetAgent {
                    account_id: agent_addr.to_string(),
                },
            )?;
        if agent_response.agent.map_or(true, |agent| {
            agent.status != croncat_sdk_agents::types::AgentStatus::Active
        }) {
            return Err(ContractError::NoTaskForAgent {});
        }

        // A hash means agent is attempting to execute evented task
        deps.querier.query_wasm_smart(
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksQueryMsg::Task { task_hash: hash },
        )?
    } else {
        // For scheduled case - check only active agents that are allowed tasks
        let agent_tasks: croncat_sdk_agents::msg::AgentTaskResponse =
            deps.querier.query_wasm_smart(
                agents_addr,
                &croncat_sdk_agents::msg::QueryMsg::GetAgentTasks {
                    account_id: agent_addr.to_string(),
                },
            )?;
        if agent_tasks.stats.num_block_tasks.is_zero() && agent_tasks.stats.num_cron_tasks.is_zero()
        {
            return Err(ContractError::NoTaskForAgent {});
        }

        // get a scheduled task
        deps.querier.query_wasm_smart(
            tasks_addr.clone(),
            &croncat_sdk_tasks::msg::TasksQueryMsg::CurrentTask {},
        )?
    };

    let Some(mut task) = current_task.task else {
        // No task
        return Err(ContractError::NoTask {});
    };

    // check if ready between boundary (if any)
    if is_before_boundary(&env.block, Some(&task.boundary)) {
        return Err(ContractError::TaskNotReady {});
    }
    if is_after_boundary(&env.block, Some(&task.boundary)) {
        // End task
        return end_task(
            deps,
            task,
            config,
            agent_addr,
            tasks_addr,
            Some(vec![Attribute::new("lifecycle", "task_ended")]),
        );
    }
    let event_based = task.queries.iter().flatten().any(|q| q.check_result);
    if event_based && !(task.interval == Interval::Once || task.interval == Interval::Immediate) {
        return Err(ContractError::TaskNoLongerValid {});
    }
    // Don't allow to execute any task available
    else if !event_based && pre_event_based {
        return Err(ContractError::NoTask {});
    }
    if let Some(queries) = task.queries.as_ref() {
        // Process all the queries
        let mut query_responses = Vec::with_capacity(queries.len());
        for query in queries {
            let query_result: StdResult<mod_sdk::types::QueryResponse> = deps.querier.query(
                &WasmQuery::Smart {
                    contract_addr: query.contract_addr.clone(),
                    msg: query.msg.clone(),
                }
                .into(),
            );
            match query_result {
                Ok(query_res) => {
                    if query.check_result && !query_res.result {
                        return Err(ContractError::TaskQueryResultFalse {});
                    }
                    query_responses.push(query_res.data);
                }
                Err(err) => {
                    return queries_transforms_fallback(
                        deps, config, task, agent_addr, tasks_addr, err,
                    );
                }
            }
        }
        if let Err(err) = replace_values(&mut task, query_responses) {
            return queries_transforms_fallback(deps, config, task, agent_addr, tasks_addr, err);
        }

        // Recalculate cw20 usage and re-check for self-calls
        let invalidated_after_transform = if let Ok(amounts) =
            recalculate_cw20(&task, &config, deps.as_ref(), &env.contract.address)
        {
            task.amount_for_one_task.cw20 = amounts;
            false
        } else {
            true
        };

        // Need to re-check if task has enough cw20's
        // because it could have been changed through transform
        let task_balance = TASKS_BALANCES.load(deps.storage, task.task_hash.as_bytes())?;
        if invalidated_after_transform
            || task_balance
                .verify_enough_cw20(task.amount_for_one_task.cw20.clone(), Uint128::new(1))
                .is_err()
        {
            // Task is no longer valid
            return end_task(
                deps,
                task,
                config,
                agent_addr,
                tasks_addr,
                Some(vec![Attribute::new("lifecycle", "task_invalidated")]),
            );
        }
    }

    let sub_msgs = task_sub_msgs(&task);
    let queue_item = QueueItem {
        task,
        agent_addr,
        failures: Default::default(),
    };

    REPLY_QUEUE.save(deps.storage, &queue_item)?;
    Ok(Response::new()
        .add_attribute("action", "proxy_call")
        .add_submessages(sub_msgs))
}

fn end_task(
    deps: DepsMut,
    task: TaskInfo,
    config: Config,
    agent_addr: Addr,
    tasks_addr: Addr,
    attrs: Option<Vec<Attribute>>,
) -> Result<Response, ContractError> {
    // Sub gas/fee from native
    let gas_with_fees = gas_with_fees(
        task.amount_for_one_task.gas,
        config.agent_fee + config.treasury_fee,
    )?;
    let native_for_gas_required = config.gas_price.calculate(gas_with_fees)?;
    let mut task_balance = TASKS_BALANCES.load(deps.storage, task.task_hash.as_bytes())?;
    task_balance.native_balance = task_balance
        .native_balance
        .checked_sub(Uint128::new(native_for_gas_required))
        .map_err(StdError::overflow)?;

    // Account for fees, to reimburse agent for efforts
    add_fee_rewards(
        deps.storage,
        task.amount_for_one_task.gas,
        &config.gas_price,
        &agent_addr,
        config.agent_fee,
        config.treasury_fee,
    )?;

    // refund the final balances to task owner
    let coins_transfer = remove_task_balance(
        deps.storage,
        task_balance,
        &task.owner_addr,
        &config.native_denom,
        task.task_hash.as_bytes(),
    )?;
    let msg = croncat_sdk_core::internal_messages::tasks::TasksRemoveTaskByManager {
        task_hash: task.task_hash.into_bytes(),
    }
    .into_cosmos_msg(tasks_addr)?;
    let bank_send = BankMsg::Send {
        to_address: task.owner_addr.into_string(),
        amount: coins_transfer,
    };
    Ok(Response::new()
        .add_attribute("action", "remove_task")
        .add_attributes(attrs.unwrap_or_default())
        .add_message(msg)
        .add_message(bank_send))
}

/// Execute: UpdateConfig
/// Used by contract owner to update config or pause contract
///
/// Returns updated [`Config`]
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    msg: UpdateConfig,
) -> Result<Response, ContractError> {
    CONFIG.update(deps.storage, |mut config| {
        // Deconstruct, so we don't miss any fields
        let UpdateConfig {
            owner_addr,
            paused,
            agent_fee,
            treasury_fee,
            gas_price,
            croncat_tasks_key,
            croncat_agents_key,
            treasury_addr,
            cw20_whitelist,
        } = msg;

        if info.sender != config.owner_addr {
            return Err(ContractError::Unauthorized {});
        }

        let gas_price = gas_price.unwrap_or(config.gas_price);
        if !gas_price.is_valid() {
            return Err(ContractError::InvalidGasPrice {});
        }

        let owner_addr = owner_addr
            .map(|human| deps.api.addr_validate(&human))
            .transpose()?
            .unwrap_or(config.owner_addr);
        let treasury_addr = if let Some(human) = treasury_addr {
            Some(deps.api.addr_validate(&human)?)
        } else {
            config.treasury_addr
        };

        let cw20_whitelist: Vec<Addr> = cw20_whitelist
            .unwrap_or_default()
            .into_iter()
            .map(|human| deps.api.addr_validate(&human))
            .collect::<StdResult<_>>()?;

        config.cw20_whitelist.extend(cw20_whitelist);

        let new_config = Config {
            paused: paused.unwrap_or(config.paused),
            owner_addr,
            croncat_factory_addr: config.croncat_factory_addr,
            croncat_tasks_key: croncat_tasks_key.unwrap_or(config.croncat_tasks_key),
            croncat_agents_key: croncat_agents_key.unwrap_or(config.croncat_agents_key),
            agent_fee: agent_fee.unwrap_or(config.agent_fee),
            treasury_fee: treasury_fee.unwrap_or(config.treasury_fee),
            gas_price,
            cw20_whitelist: config.cw20_whitelist,
            native_denom: config.native_denom,
            limit: config.limit,
            treasury_addr,
        };
        Ok(new_config)
    })?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

fn execute_create_task_balance(
    deps: DepsMut,
    info: MessageInfo,
    msg: ManagerCreateTaskBalance,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    check_if_sender_is_tasks(&deps.querier, &config, &info.sender)?;
    let (native, ibc) = attached_natives(&config.native_denom, info.funds)?;
    let cw20 = msg.cw20;
    if let Some(attached_cw20) = &cw20 {
        sub_user_cw20(deps.storage, &msg.sender, attached_cw20)?;
    }
    let tasks_balance = TaskBalance {
        native_balance: native,
        cw20_balance: cw20,
        ibc_balance: ibc,
    };
    // Let's check if task has enough attached balance
    {
        let gas_with_fees = gas_with_fees(
            msg.amount_for_one_task.gas,
            config.agent_fee + config.treasury_fee,
        )?;
        let native_for_gas_required = config.gas_price.calculate(gas_with_fees)?;
        let (native_for_sends_required, ibc_required) =
            calculate_required_natives(msg.amount_for_one_task.coin, &config.native_denom)?;
        tasks_balance.verify_enough_attached(
            Uint128::from(native_for_gas_required) + native_for_sends_required,
            msg.amount_for_one_task.cw20,
            ibc_required,
            msg.recurring,
            &config.native_denom,
        )?;
    }
    TASKS_BALANCES.save(deps.storage, &msg.task_hash, &tasks_balance)?;

    Ok(Response::new().add_attribute("action", "create_task_balance"))
}

/// Allows an agent to withdraw all rewards, paid to the specified payable account id.
fn execute_withdraw_agent_rewards(
    deps: DepsMut,
    info: MessageInfo,
    args: Option<AgentWithdrawOnRemovalArgs>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    //assert if contract is ready for execution
    check_ready_for_execution(&info, &config)?;

    let agent_id: Addr;
    let payable_account_id: Addr;
    let mut fail_on_zero_balance = true;

    if let Some(arg) = args {
        assert_caller_is_agent_contract(&deps.querier, &config, &info.sender)?;
        agent_id = Addr::unchecked(arg.agent_id);
        payable_account_id = Addr::unchecked(arg.payable_account_id);
        fail_on_zero_balance = false;
    } else {
        agent_id = info.sender;
        let agent = query_agent(&deps.querier, &config, agent_id.to_string())?
            .agent
            .ok_or(ContractError::NoRewardsOwnerAgentFound {})?;
        payable_account_id = agent.payable_account_id;
    }
    let agent_rewards = AGENT_REWARDS
        .may_load(deps.storage, &agent_id)?
        .unwrap_or_default();

    AGENT_REWARDS.remove(deps.storage, &agent_id);

    let mut msgs = vec![];
    // This will send all token balances to Agent
    let msg = create_bank_send_message(
        &payable_account_id,
        &config.native_denom,
        agent_rewards.u128(),
    )?;

    if !agent_rewards.is_zero() {
        msgs.push(msg);
    } else if fail_on_zero_balance {
        return Err(ContractError::NoWithdrawRewardsAvailable {});
    }

    Ok(Response::new()
        .add_messages(msgs)
        .set_data(to_binary(&AgentWithdrawCallback {
            agent_id: agent_id.to_string(),
            amount: agent_rewards,
            payable_account_id: payable_account_id.to_string(),
        })?)
        .add_attribute("action", "withdraw_rewards")
        .add_attribute("payment_account_id", &payable_account_id)
        .add_attribute("rewards", agent_rewards))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::TreasuryBalance {} => to_binary(&TREASURY_BALANCE.load(deps.storage)?),
        QueryMsg::UsersBalances {
            address,
            from_index,
            limit,
        } => to_binary(&query_users_balances(deps, address, from_index, limit)?),
        QueryMsg::TaskBalance { task_hash } => to_binary(&TaskBalanceResponse {
            balance: TASKS_BALANCES.may_load(deps.storage, task_hash.as_bytes())?,
        }),
        QueryMsg::AgentRewards { agent_id } => to_binary(
            &AGENT_REWARDS
                .may_load(deps.storage, &Addr::unchecked(agent_id))?
                .unwrap_or(Uint128::zero()),
        ),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        TASK_REPLY => {
            let execute_data = parse_reply_execute_data(msg)?;
            let remove_task_msg: Option<ManagerRemoveTask> =
                from_binary(&execute_data.data.unwrap())?;
            let Some(msg) = remove_task_msg else {
                return Ok(Response::new())
            };
            let config = CONFIG.load(deps.storage)?;
            let task_owner = msg.sender;
            let task_balance = TASKS_BALANCES.load(deps.storage, &msg.task_hash)?;
            let coins_transfer = remove_task_balance(
                deps.storage,
                task_balance,
                &task_owner,
                &config.native_denom,
                &msg.task_hash,
            )?;

            let bank_send = BankMsg::Send {
                to_address: task_owner.into_string(),
                amount: coins_transfer,
            };
            Ok(Response::new().add_message(bank_send))
        }
        _ => {
            let mut queue_item = REPLY_QUEUE.load(deps.storage)?;
            let last = parse_reply_msg(deps.storage, &mut queue_item, msg);
            if last {
                let failures: Vec<Attribute> = queue_item
                    .failures
                    .iter()
                    .map(|(idx, failure)| Attribute::new(format!("action{}_failure", idx), failure))
                    .collect();
                let config = CONFIG.load(deps.storage)?;
                let complete_msg = create_task_completed_msg(
                    &deps.querier,
                    &config,
                    &queue_item.agent_addr,
                    !matches!(queue_item.task.interval, Interval::Cron(_)),
                )?;
                Ok(finalize_task(deps, queue_item)?
                    .add_message(complete_msg)
                    .add_attributes(failures))
            } else {
                Ok(Response::new())
            }
        }
    }
}
