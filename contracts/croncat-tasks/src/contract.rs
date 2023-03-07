use crate::ContractError::InvalidZeroValue;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Attribute, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult,
    Uint64,
};
use croncat_sdk_core::internal_messages::agents::AgentOnTaskCreated;
use croncat_sdk_core::internal_messages::manager::{ManagerCreateTaskBalance, ManagerRemoveTask};
use croncat_sdk_core::internal_messages::tasks::{TasksRemoveTaskByManager, TasksRescheduleTask};
use croncat_sdk_core::types::{DEFAULT_PAGINATION_FROM_INDEX, DEFAULT_PAGINATION_LIMIT};
use croncat_sdk_tasks::msg::UpdateConfigMsg;
use croncat_sdk_tasks::types::{
    Config, CurrentTaskInfoResponse, Interval, SlotHashesResponse, SlotIdsResponse,
    SlotTasksTotalResponse, SlotType, Task, TaskInfo, TaskRequest, TaskResponse,
};
use cw2::set_contract_version;
use cw20::Cw20CoinVerified;
use cw_storage_plus::Bound;

use crate::error::ContractError;
use crate::helpers::{
    check_if_sender_is_manager, get_agents_addr, get_manager_addr, remove_task, validate_boundary,
    validate_msg_calculate_usage,
};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    tasks_map, BLOCK_SLOTS, CONFIG, EVENTED_TASKS_LOOKUP, LAST_TASK_CREATION, PAUSED, TASKS_TOTAL,
    TIME_SLOTS,
};

const CONTRACT_NAME: &str = "crate:croncat-tasks";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Default value based on non-wasm operations, wasm ops seem impossible to predict
// TODO: this values based of pre-split, need to recalculate GAS_BASE_FEE
pub(crate) const GAS_BASE_FEE: u64 = 300_000;
pub(crate) const GAS_ACTION_FEE: u64 = 130_000;
pub(crate) const GAS_QUERY_FEE: u64 = 130_000; // Load query module(~61_000) and query after that(~65_000+)
pub(crate) const GAS_LIMIT: u64 = 3_000_000; // 10M is default for juno, but let's make sure we have space for block inclusivity guarantees
pub(crate) const SLOT_GRANULARITY_TIME: u64 = 10_000_000_000; // 10 seconds

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let InstantiateMsg {
        chain_name,
        version,
        pause_admin,
        croncat_manager_key,
        croncat_agents_key,
        slot_granularity_time,
        gas_base_fee,
        gas_action_fee,
        gas_limit,
        gas_query_fee,
    } = msg;

    validate_non_zero_value(slot_granularity_time, "slot_granularity_time")?;

    let contract_version = version.unwrap_or_else(|| CONTRACT_VERSION.to_string());
    set_contract_version(deps.storage, CONTRACT_NAME, &contract_version)?;

    let owner_addr = info.sender.clone();

    // Validate pause_admin
    // MUST: only be contract address
    // MUST: not be same address as factory owner (DAO)
    // Any factory action should be done by the owner_addr
    let pause_addr = deps.api.addr_validate(pause_admin.as_str())?;
    if owner_addr == pause_addr || !(63usize..=64usize).contains(&pause_addr.to_string().len()) {
        return Err(ContractError::InvalidPauseAdmin {});
    }

    let config = Config {
        chain_name,
        version: contract_version,
        owner_addr,
        pause_admin,
        croncat_factory_addr: info.sender,
        croncat_manager_key,
        croncat_agents_key,
        slot_granularity_time: slot_granularity_time.unwrap_or(SLOT_GRANULARITY_TIME),
        gas_base_fee: gas_base_fee.unwrap_or(GAS_BASE_FEE),
        gas_action_fee: gas_action_fee.unwrap_or(GAS_ACTION_FEE),
        gas_query_fee: gas_query_fee.unwrap_or(GAS_QUERY_FEE),
        gas_limit: gas_limit.unwrap_or(GAS_LIMIT),
    };

    // Ensure the new gas limit will work
    validate_gas_limit(&config)?;

    // Save initializing states
    CONFIG.save(deps.storage, &config)?;
    PAUSED.save(deps.storage, &false)?;
    TASKS_TOTAL.save(deps.storage, &0)?;
    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig(msg) => execute_update_config(deps, info, msg),
        ExecuteMsg::CreateTask { task } => execute_create_task(deps, env, info, *task),
        ExecuteMsg::RemoveTask { task_hash } => execute_remove_task(deps, info, task_hash),
        // Methods for other contracts
        ExecuteMsg::RemoveTaskByManager(remove_task_msg) => {
            execute_remove_task_by_manager(deps, info, remove_task_msg)
        }
        ExecuteMsg::RescheduleTask(reschedule_msg) => {
            execute_reschedule_task(deps, env, info, reschedule_msg)
        }
        ExecuteMsg::PauseContract {} => execute_pause(deps, info),
        ExecuteMsg::UnpauseContract {} => execute_unpause(deps, info),
    }
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    msg: UpdateConfigMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner_addr {
        return Err(ContractError::Unauthorized {});
    }

    // Destruct so we won't forget to update if if new fields added
    let UpdateConfigMsg {
        croncat_factory_addr,
        croncat_manager_key,
        croncat_agents_key,
        slot_granularity_time,
        gas_base_fee,
        gas_action_fee,
        gas_query_fee,
        gas_limit,
    } = msg;

    let new_config = Config {
        owner_addr: config.owner_addr,
        pause_admin: config.pause_admin,
        croncat_factory_addr: croncat_factory_addr
            .map(|human| deps.api.addr_validate(&human))
            .transpose()?
            .unwrap_or(config.croncat_factory_addr),
        chain_name: config.chain_name,
        version: config.version,
        croncat_manager_key: croncat_manager_key.unwrap_or(config.croncat_manager_key),
        croncat_agents_key: croncat_agents_key.unwrap_or(config.croncat_agents_key),
        slot_granularity_time: slot_granularity_time.unwrap_or(config.slot_granularity_time),
        gas_base_fee: gas_base_fee.unwrap_or(config.gas_base_fee),
        gas_action_fee: gas_action_fee.unwrap_or(config.gas_action_fee),
        gas_query_fee: gas_query_fee.unwrap_or(config.gas_query_fee),
        gas_limit: gas_limit.unwrap_or(config.gas_limit),
    };

    // Ensure the new gas limit will work
    validate_gas_limit(&new_config)?;
    validate_non_zero_value(slot_granularity_time, "slot_granularity_time")?;

    CONFIG.save(deps.storage, &new_config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

fn execute_reschedule_task(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    reschedule_msg: TasksRescheduleTask,
) -> Result<Response, ContractError> {
    let task_hash = reschedule_msg.task_hash;
    let config = CONFIG.load(deps.storage)?;
    check_if_sender_is_manager(&deps.querier, &config, &info.sender)?;

    let mut task_to_remove = None;
    // Check default map
    let (next_id, slot_kind) = if let Some(task) = tasks_map().may_load(deps.storage, &task_hash)? {
        let (next_id, slot_kind) =
            task.interval
                .next(&env, &task.boundary, config.slot_granularity_time);

        // NOTE: If task is evented, we dont want to "schedule" inside slots
        // but we also dont want to remove unless it was Interval::Once
        if next_id != 0 && !task.is_evented() && task.interval != Interval::Once {
            // Get previous task hashes in slot, add as needed
            let update_vec_data = |d: Option<Vec<Vec<u8>>>| -> StdResult<Vec<Vec<u8>>> {
                match d {
                    // has some data, simply push new hash
                    Some(data) => {
                        let mut s = data;
                        s.push(task_hash);
                        Ok(s)
                    }
                    // No data, push new vec & hash
                    None => Ok(vec![task_hash]),
                }
            };
            // Based on slot kind, put into block or cron slots
            match slot_kind {
                SlotType::Block => {
                    BLOCK_SLOTS.update(deps.storage, next_id, update_vec_data)?;
                    // Don't forget to pop finished task
                    let mut block_slot: Vec<(u64, Vec<Vec<u8>>)> = BLOCK_SLOTS
                        .range(
                            deps.storage,
                            None,
                            Some(Bound::inclusive(env.block.height)),
                            Order::Ascending,
                        )
                        .take(1)
                        .collect::<StdResult<_>>()?;
                    let mut slot = block_slot.pop().unwrap();
                    slot.1.pop();
                    if slot.1.is_empty() {
                        BLOCK_SLOTS.remove(deps.storage, slot.0)
                    } else {
                        BLOCK_SLOTS.save(deps.storage, slot.0, &slot.1)?;
                    }
                }
                SlotType::Cron => {
                    TIME_SLOTS.update(deps.storage, next_id, update_vec_data)?;
                    // Don't forget to pop finished task
                    let mut time_slot: Vec<(u64, Vec<Vec<u8>>)> = TIME_SLOTS
                        .range(
                            deps.storage,
                            None,
                            Some(Bound::inclusive(env.block.time.nanos())),
                            Order::Ascending,
                        )
                        .take(1)
                        .collect::<StdResult<_>>()?;
                    let mut slot = time_slot.pop().unwrap();
                    slot.1.pop();
                    if slot.1.is_empty() {
                        TIME_SLOTS.remove(deps.storage, slot.0)
                    } else {
                        TIME_SLOTS.save(deps.storage, slot.0, &slot.1)?;
                    }
                }
            }
        } else if !task.is_evented() {
            remove_task(
                deps.storage,
                &task_hash,
                task.boundary.is_block(),
                task.is_evented(),
            )?;
            task_to_remove = Some(ManagerRemoveTask {
                sender: task.owner_addr,
                task_hash,
            });
        }
        (next_id, slot_kind)
    } else {
        return Err(ContractError::NoTaskFound {});
    };

    Ok(Response::new()
        .add_attribute("action", "reschedule_task")
        .add_attribute("slot_id", next_id.to_string())
        .add_attribute("slot_kind", slot_kind.to_string())
        .set_data(to_binary(&task_to_remove)?))
}

fn execute_remove_task_by_manager(
    deps: DepsMut,
    info: MessageInfo,
    remove_task_msg: TasksRemoveTaskByManager,
) -> Result<Response, ContractError> {
    let task_hash = remove_task_msg.task_hash;
    let config = CONFIG.load(deps.storage)?;
    check_if_sender_is_manager(&deps.querier, &config, &info.sender)?;

    if let Some(task) = tasks_map().may_load(deps.storage, &task_hash)? {
        remove_task(
            deps.storage,
            &task_hash,
            task.boundary.is_block(),
            task.is_evented(),
        )?;
    } else {
        return Err(ContractError::NoTaskFound {});
    }

    Ok(Response::new().add_attribute("action", "remove_task_by_manager"))
}

fn execute_remove_task(
    deps: DepsMut,
    info: MessageInfo,
    task_hash: String,
) -> Result<Response, ContractError> {
    if PAUSED.load(deps.storage)? {
        return Err(ContractError::ContractPaused);
    }
    let config = CONFIG.load(deps.storage)?;
    let hash = task_hash.as_bytes();
    if let Some(task) = tasks_map().may_load(deps.storage, hash)? {
        if task.owner_addr != info.sender {
            return Err(ContractError::Unauthorized {});
        }
        remove_task(
            deps.storage,
            hash,
            task.boundary.is_block(),
            task.is_evented(),
        )?;
    } else {
        return Err(ContractError::NoTaskFound {});
    }
    let manager_addr = get_manager_addr(&deps.querier, &config)?;
    let remove_task_msg = ManagerRemoveTask {
        sender: info.sender,
        task_hash: task_hash.into_bytes(),
    }
    .into_cosmos_msg(manager_addr)?;
    Ok(Response::new()
        .add_attribute("action", "remove_task")
        .add_message(remove_task_msg))
}

fn execute_create_task(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    task: TaskRequest,
) -> Result<Response, ContractError> {
    if PAUSED.load(deps.storage)? {
        return Err(ContractError::ContractPaused);
    }
    let config = CONFIG.load(deps.storage)?;
    let owner_addr = info.sender;

    // Validate boundary and interval
    let boundary = validate_boundary(&env.block, task.boundary.clone(), &task.interval)?;

    let amount_for_one_task = validate_msg_calculate_usage(
        deps.as_ref(),
        &task,
        &env.contract.address,
        &owner_addr,
        &config,
    )?;
    if amount_for_one_task.gas > config.gas_limit {
        return Err(ContractError::InvalidGas {});
    }
    let cw20 = task
        .cw20
        .map(|human| {
            StdResult::Ok(Cw20CoinVerified {
                address: deps.api.addr_validate(&human.address)?,
                amount: human.amount,
            })
        })
        .transpose()?;

    let item = Task {
        owner_addr: owner_addr.clone(),
        interval: task.interval,
        boundary,
        stop_on_fail: task.stop_on_fail,
        amount_for_one_task: amount_for_one_task.clone(),
        actions: task.actions,
        queries: task.queries.unwrap_or_default(),
        transforms: task.transforms.unwrap_or_default(),
        version: config.version.clone(),
    };
    let event_based = item.is_evented();
    if !item.interval.is_valid()
        || (event_based
            && !(item.interval == Interval::Once || item.interval == Interval::Immediate))
    {
        return Err(ContractError::InvalidInterval {});
    }

    let hash_prefix = &config.chain_name;
    let hash = item.to_hash(hash_prefix);

    let (next_id, slot_kind) =
        item.interval
            .next(&env, &item.boundary, config.slot_granularity_time);
    if next_id == 0 {
        return Err(ContractError::TaskEnded {});
    }

    let recurring = item.recurring();
    let hash_vec = hash.clone().into_bytes();
    let mut attributes: Vec<Attribute> = vec![];

    // Update query totals and map
    TASKS_TOTAL.update(deps.storage, |amt| -> StdResult<_> { Ok(amt + 1) })?;
    tasks_map().update(deps.storage, &hash_vec, |old| match old {
        Some(_) => Err(ContractError::TaskExists {}),
        None => Ok(item.clone()),
    })?;

    // Get previous task hashes in slot, add as needed
    let update_vec_data = |d: Option<Vec<Vec<u8>>>| -> StdResult<Vec<Vec<u8>>> {
        match d {
            // has some data, simply push new hash
            Some(data) => {
                let mut s = data;
                s.push(hash_vec.clone());
                Ok(s)
            }
            // No data, push new vec & hash
            None => Ok(vec![hash_vec.clone()]),
        }
    };

    if event_based {
        EVENTED_TASKS_LOOKUP.update(deps.storage, next_id, update_vec_data)?;
        attributes.push(Attribute::new("evented_id", next_id.to_string()));
    } else {
        // Only scheduled tasks get put into slots
        match slot_kind {
            SlotType::Block => {
                BLOCK_SLOTS.update(deps.storage, next_id, update_vec_data)?;
            }
            SlotType::Cron => {
                TIME_SLOTS.update(deps.storage, next_id, update_vec_data)?;
            }
        }
        attributes.push(Attribute::new("slot_id", next_id.to_string()));
        attributes.push(Attribute::new("slot_kind", slot_kind.to_string()));
    }

    // Save the current timestamp as the last time a task was created
    LAST_TASK_CREATION.save(deps.storage, &env.block.time)?;

    let manager_addr = get_manager_addr(&deps.querier, &config)?;
    let manager_create_task_balance_msg = ManagerCreateTaskBalance {
        sender: owner_addr,
        task_hash: hash_vec,
        recurring,
        cw20,
        amount_for_one_task,
    }
    .into_cosmos_msg(manager_addr, info.funds)?;

    let agent_addr = get_agents_addr(&deps.querier, &config)?;
    let agent_new_task_msg = AgentOnTaskCreated {}.into_cosmos_msg(agent_addr)?;
    Ok(Response::new()
        .set_data(hash.as_bytes())
        .add_attribute("action", "create_task")
        .add_attributes(attributes)
        .add_attribute("task_hash", hash)
        .add_message(manager_create_task_balance_msg)
        .add_message(agent_new_task_msg))
}

pub fn execute_pause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    if PAUSED.load(deps.storage)? {
        return Err(ContractError::ContractPaused);
    }
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.pause_admin {
        return Err(ContractError::Unauthorized {});
    }
    PAUSED.save(deps.storage, &true)?;
    Ok(Response::new().add_attribute("action", "pause_contract"))
}

pub fn execute_unpause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    if !PAUSED.load(deps.storage)? {
        return Err(ContractError::ContractUnpaused);
    }
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner_addr {
        return Err(ContractError::Unauthorized {});
    }
    PAUSED.save(deps.storage, &false)?;
    Ok(Response::new().add_attribute("action", "unpause_contract"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::Paused {} => to_binary(&PAUSED.load(deps.storage)?),
        QueryMsg::TasksTotal {} => to_binary(&cosmwasm_std::Uint64::from(query_tasks_total(deps)?)),
        QueryMsg::CurrentTaskInfo {} => to_binary(&query_current_task_info(deps, env)?),
        QueryMsg::CurrentTask {} => to_binary(&query_current_task(deps, env)?),
        QueryMsg::Tasks { from_index, limit } => to_binary(&query_tasks(deps, from_index, limit)?),
        QueryMsg::EventedIds { from_index, limit } => {
            to_binary(&query_evented_ids(deps, from_index, limit)?)
        }
        QueryMsg::EventedHashes {
            id,
            from_index,
            limit,
        } => to_binary(&query_evented_hashes(deps, id, from_index, limit)?),
        QueryMsg::EventedTasks {
            start,
            from_index,
            limit,
        } => to_binary(&query_evented_tasks(deps, env, start, from_index, limit)?),
        QueryMsg::TasksByOwner {
            owner_addr,
            from_index,
            limit,
        } => to_binary(&query_tasks_by_owner(deps, owner_addr, from_index, limit)?),
        QueryMsg::Task { task_hash } => to_binary(&query_task(deps, task_hash)?),
        QueryMsg::TaskHash { task } => to_binary(&query_task_hash(deps, *task)?),
        QueryMsg::SlotHashes { slot } => to_binary(&query_slot_hashes(deps, slot)?),
        QueryMsg::SlotIds { from_index, limit } => {
            to_binary(&query_slot_ids(deps, from_index, limit)?)
        }
        QueryMsg::SlotTasksTotal { offset } => {
            to_binary(&query_slot_tasks_total(deps, env, offset)?)
        }
    }
}

fn query_tasks_total(deps: Deps) -> StdResult<u64> {
    TASKS_TOTAL.load(deps.storage)
}

// returns the total task count & last task creation timestamp for agent nomination checks
fn query_current_task_info(deps: Deps, _env: Env) -> StdResult<CurrentTaskInfoResponse> {
    Ok(CurrentTaskInfoResponse {
        total: Uint64::from(query_tasks_total(deps).unwrap()),
        last_created_task: LAST_TASK_CREATION.load(deps.storage)?,
    })
}

fn query_slot_tasks_total(
    deps: Deps,
    env: Env,
    offset: Option<u64>,
) -> StdResult<SlotTasksTotalResponse> {
    if let Some(off) = offset {
        let config = CONFIG.load(deps.storage)?;
        let block_tasks = BLOCK_SLOTS
            .may_load(deps.storage, env.block.height + off)?
            .unwrap_or_default()
            .len() as u64;
        let evented_tasks = EVENTED_TASKS_LOOKUP
            .may_load(deps.storage, env.block.height + off)?
            .unwrap_or_default()
            .len() as u64;

        let current_block_ts = env.block.time.nanos();
        let current_block_slot =
            current_block_ts.saturating_sub(current_block_ts % config.slot_granularity_time);
        let cron_tasks = TIME_SLOTS
            .may_load(
                deps.storage,
                current_block_slot + config.slot_granularity_time * off,
            )?
            .unwrap_or_default()
            .len() as u64;
        Ok(SlotTasksTotalResponse {
            block_tasks,
            cron_tasks,
            evented_tasks,
        })
    } else {
        let block_slots: Vec<(u64, Vec<Vec<u8>>)> = BLOCK_SLOTS
            .range(
                deps.storage,
                None,
                Some(Bound::inclusive(env.block.height)),
                Order::Ascending,
            )
            .collect::<StdResult<_>>()?;

        let block_tasks = block_slots
            .iter()
            .fold(0, |acc, (_, hashes)| acc + hashes.len()) as u64;

        let evented_task_list: Vec<(u64, Vec<Vec<u8>>)> = EVENTED_TASKS_LOOKUP
            .range(
                deps.storage,
                None,
                Some(Bound::inclusive(env.block.height)),
                Order::Ascending,
            )
            .collect::<StdResult<_>>()?;

        let evented_tasks = evented_task_list
            .iter()
            .fold(0, |acc, (_, hashes)| acc + hashes.len()) as u64;

        let time_slot: Vec<(u64, Vec<Vec<u8>>)> = TIME_SLOTS
            .range(
                deps.storage,
                None,
                Some(Bound::inclusive(env.block.time.nanos())),
                Order::Ascending,
            )
            .collect::<StdResult<_>>()?;

        let cron_tasks = time_slot
            .iter()
            .fold(0, |acc, (_, hashes)| acc + hashes.len()) as u64;
        Ok(SlotTasksTotalResponse {
            block_tasks,
            cron_tasks,
            evented_tasks,
        })
    }
}

/// Get the slot with lowest height/timestamp
/// NOTE: This prioritizes blocks over timestamps
/// Why blocks over timestamps? Time-based tasks have a wider granularity than blocks.
/// For example, the default configuration for time granularity is 10 seconds. This means
/// on average a time-based task could occur on 1 of 2 blocks. This configuration is aimed
/// at a larger window of time because timestamp guarantees are virtually impossible, without
/// protocol level mechanisms. Since this is the case, timestamps should be considered more
/// flexible in execution windows. Future versions of this contract can ensure better
/// execution guarantees, based on whether the timestamp is nearing the end of its block-span
/// window (timestamp end is closer to 2nd block than 1st).
fn query_current_task(deps: Deps, env: Env) -> StdResult<TaskResponse> {
    let config = CONFIG.load(deps.storage)?;
    let mut block_slot: Vec<(u64, Vec<Vec<u8>>)> = BLOCK_SLOTS
        .range(
            deps.storage,
            None,
            Some(Bound::inclusive(env.block.height)),
            Order::Ascending,
        )
        .take(1)
        .collect::<StdResult<_>>()?;
    if !block_slot.is_empty() {
        let task_hash = block_slot.pop().unwrap().1.pop().unwrap();
        let task = tasks_map().load(deps.storage, &task_hash)?;
        Ok(task.into_response(&config.chain_name))
    } else {
        let mut time_slot: Vec<(u64, Vec<Vec<u8>>)> = TIME_SLOTS
            .range(
                deps.storage,
                None,
                Some(Bound::inclusive(env.block.time.nanos())),
                Order::Ascending,
            )
            .take(1)
            .collect::<StdResult<_>>()?;
        if !time_slot.is_empty() {
            let task_hash = time_slot.pop().unwrap().1.pop().unwrap();
            let task = tasks_map().load(deps.storage, &task_hash)?;
            Ok(task.into_response(&config.chain_name))
        } else {
            Ok(TaskResponse { task: None })
        }
    }
}

fn query_tasks(
    deps: Deps,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<TaskInfo>> {
    let config = CONFIG.load(deps.storage)?;

    let from_index = from_index.unwrap_or_default();
    let limit = limit.unwrap_or(100);

    tasks_map()
        .range(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .map(|task_res| {
            task_res.map(|(_, task)| task.into_response(&config.chain_name).task.unwrap())
        })
        .collect()
}

fn query_evented_tasks(
    deps: Deps,
    env: Env,
    start: Option<u64>,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<TaskInfo>> {
    let config = CONFIG.load(deps.storage)?;
    let from_index = from_index.unwrap_or(DEFAULT_PAGINATION_FROM_INDEX);
    let limit = limit.unwrap_or(DEFAULT_PAGINATION_LIMIT);
    let tm = tasks_map();

    let mut evented_hashes: Vec<Vec<u8>> = Vec::new();
    let mut all_tasks: Vec<TaskInfo> = Vec::new();

    // Check if start was supplied, otherwise get the next ids for block and time
    if let Some(i) = start {
        evented_hashes = EVENTED_TASKS_LOOKUP
            .may_load(deps.storage, i)?
            .unwrap_or_default();
    } else {
        let block_evented: Vec<(u64, _)> = EVENTED_TASKS_LOOKUP
            .range(
                deps.storage,
                Some(Bound::inclusive(env.block.height)),
                None,
                Order::Ascending,
            )
            .skip(from_index as usize)
            .take(limit as usize)
            .collect::<StdResult<Vec<(u64, _)>>>()?;

        if !block_evented.is_empty() {
            for item in block_evented {
                evented_hashes = [evented_hashes, item.1].concat();
            }
        }

        let time_evented: Vec<(u64, _)> = EVENTED_TASKS_LOOKUP
            .range(
                deps.storage,
                Some(Bound::inclusive(env.block.time.nanos())),
                None,
                Order::Ascending,
            )
            .take(1)
            .collect::<StdResult<Vec<(u64, _)>>>()?;

        if !time_evented.is_empty() {
            for item in time_evented {
                evented_hashes = [evented_hashes, item.1].concat();
            }
        }
    }

    // Loop and get all associated tasks by hash
    for t in evented_hashes {
        if let Some(task) = tm.may_load(deps.storage, &t)? {
            all_tasks.push(task.into_response(&config.chain_name).task.unwrap())
        }
    }

    Ok(all_tasks)
}

fn query_evented_ids(
    deps: Deps,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<u64>> {
    let from_index = from_index.unwrap_or(DEFAULT_PAGINATION_FROM_INDEX);
    let limit = limit.unwrap_or(DEFAULT_PAGINATION_LIMIT);

    let evented_ids = EVENTED_TASKS_LOOKUP
        .keys(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .collect::<StdResult<_>>()?;

    Ok(evented_ids)
}

fn query_evented_hashes(
    deps: Deps,
    id: Option<u64>,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<String>> {
    let mut evented_hashes: Vec<Vec<u8>> = Vec::new();
    let from_index = from_index.unwrap_or(DEFAULT_PAGINATION_FROM_INDEX);
    let limit = limit.unwrap_or(DEFAULT_PAGINATION_LIMIT);

    // Check if slot was supplied, otherwise get the next slots for block and time
    if let Some(i) = id {
        evented_hashes = EVENTED_TASKS_LOOKUP
            .may_load(deps.storage, i)?
            .unwrap_or_default();
    } else {
        let evented: Vec<(u64, _)> = EVENTED_TASKS_LOOKUP
            .range(deps.storage, None, None, Order::Ascending)
            .skip(from_index as usize)
            .take(limit as usize)
            .collect::<StdResult<Vec<(u64, _)>>>()?;

        if !evented.is_empty() {
            for item in evented {
                evented_hashes = [evented_hashes, item.1].concat();
            }
        }
    }

    // Generate strings for all hashes
    let evented_task_hashes: Vec<_> = evented_hashes
        .iter()
        .map(|t| String::from_utf8(t.to_vec()).unwrap_or_else(|_| "".to_string()))
        .collect();

    Ok(evented_task_hashes)
}

fn query_tasks_by_owner(
    deps: Deps,
    owner_addr: String,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<TaskInfo>> {
    let owner_addr = deps.api.addr_validate(&owner_addr)?;
    let config = CONFIG.load(deps.storage)?;

    let from_index = from_index.unwrap_or_default();
    let limit = limit.unwrap_or(100);

    tasks_map()
        .idx
        .owner
        .prefix(owner_addr)
        .range(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .map(|task_res| {
            task_res.map(|(_, task)| task.into_response(&config.chain_name).task.unwrap())
        })
        .collect()
}

fn query_task(deps: Deps, task_hash: String) -> StdResult<TaskResponse> {
    let config = CONFIG.load(deps.storage)?;

    if let Some(task) = tasks_map().may_load(deps.storage, task_hash.as_bytes())? {
        Ok(task.into_response(&config.chain_name))
    } else {
        Ok(TaskResponse { task: None })
    }
}

fn query_task_hash(deps: Deps, task: Task) -> StdResult<String> {
    let config = CONFIG.load(deps.storage)?;
    Ok(task.to_hash(&config.chain_name))
}

fn query_slot_hashes(deps: Deps, slot: Option<u64>) -> StdResult<SlotHashesResponse> {
    let mut block_id: u64 = 0;
    let mut block_hashes: Vec<Vec<u8>> = Vec::new();
    let mut time_id: u64 = 0;
    let mut time_hashes: Vec<Vec<u8>> = Vec::new();

    // Check if slot was supplied, otherwise get the next slots for block and time
    if let Some(id) = slot {
        block_hashes = BLOCK_SLOTS.may_load(deps.storage, id)?.unwrap_or_default();
        if !block_hashes.is_empty() {
            block_id = id;
        }
        time_hashes = TIME_SLOTS.may_load(deps.storage, id)?.unwrap_or_default();
        if !time_hashes.is_empty() {
            time_id = id;
        }
    } else {
        let time: Vec<(u64, _)> = TIME_SLOTS
            .range(deps.storage, None, None, Order::Ascending)
            .take(1)
            .collect::<StdResult<Vec<(u64, _)>>>()?;

        if !time.is_empty() {
            let slot = time[0].clone();
            time_id = slot.0;
            time_hashes = slot.1;
        }

        let block: Vec<(u64, _)> = BLOCK_SLOTS
            .range(deps.storage, None, None, Order::Ascending)
            .take(1)
            .collect::<StdResult<Vec<(u64, _)>>>()?;

        if !block.is_empty() {
            let slot = block[0].clone();
            block_id = slot.0;
            block_hashes = slot.1;
        }
    }

    // Generate strings for all hashes
    let block_task_hash: Vec<_> = block_hashes
        .iter()
        .map(|b| String::from_utf8(b.to_vec()).unwrap_or_else(|_| "".to_string()))
        .collect();
    let time_task_hash: Vec<_> = time_hashes
        .iter()
        .map(|t| String::from_utf8(t.to_vec()).unwrap_or_else(|_| "".to_string()))
        .collect();

    Ok(SlotHashesResponse {
        block_id,
        block_task_hash,
        time_id,
        time_task_hash,
    })
}

fn query_slot_ids(
    deps: Deps,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<SlotIdsResponse> {
    let from_index = from_index.unwrap_or_default();
    let limit = limit.unwrap_or(100);

    let time_ids = TIME_SLOTS
        .keys(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .collect::<StdResult<_>>()?;
    let block_ids = BLOCK_SLOTS
        .keys(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .collect::<StdResult<_>>()?;

    Ok(SlotIdsResponse {
        time_ids,
        block_ids,
    })
}

fn validate_non_zero_value(opt_num: Option<u64>, field_name: &str) -> Result<(), ContractError> {
    if let Some(num) = opt_num {
        if num == 0u64 {
            Err(InvalidZeroValue {
                field: field_name.to_string(),
            })
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

/// Before changing the configuration's gas_limit, ensure it's a reasonable
/// value, meaning higher than the sum of its needed parts
fn validate_gas_limit(config: &Config) -> Result<(), ContractError> {
    if config.gas_limit > config.gas_base_fee + config.gas_action_fee + config.gas_query_fee {
        Ok(())
    } else {
        // Must be greater than those params
        Err(ContractError::InvalidGas {})
    }
}
