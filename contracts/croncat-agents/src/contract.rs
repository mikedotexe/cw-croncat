use crate::distributor::*;
use crate::error::ContractError;
use crate::error::ContractError::InvalidConfigurationValue;
use crate::external::*;
use crate::msg::*;
use crate::state::*;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    has_coins, to_binary, Addr, Attribute, Binary, Coin, Deps, DepsMut, Empty, Env, MessageInfo,
    QuerierWrapper, Response, StdError, StdResult, Storage, Uint64,
};
use croncat_sdk_agents::msg::{
    AgentInfo, AgentResponse, AgentTaskResponse, GetAgentIdsResponse, TaskStats, UpdateConfig,
};
use croncat_sdk_agents::types::{Agent, AgentNominationStatus, AgentStatus, Config};
use croncat_sdk_core::internal_messages::agents::{AgentOnTaskCompleted, AgentOnTaskCreated};
use croncat_sdk_core::types::{DEFAULT_PAGINATION_FROM_INDEX, DEFAULT_PAGINATION_LIMIT};
use cw2::set_contract_version;
use std::cmp::min;

pub(crate) const CONTRACT_NAME: &str = "crate:croncat-agents";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let InstantiateMsg {
        pause_admin,
        version,
        croncat_manager_key,
        croncat_tasks_key,
        agent_nomination_duration,
        min_tasks_per_agent,
        min_coins_for_agent_registration,
        agents_eject_threshold,
        min_active_agent_count,
    } = msg;

    validate_config_non_zero_u16(agent_nomination_duration, "agent_nomination_duration")?;
    validate_config_non_zero_u16(min_active_agent_count, "min_active_agent_count")?;
    validate_config_non_zero_u64(min_tasks_per_agent, "min_tasks_per_agent")?;
    validate_config_non_zero_u64(agents_eject_threshold, "agents_eject_threshold")?;
    validate_config_non_zero_u64(
        min_coins_for_agent_registration,
        "min_coins_for_agent_registration",
    )?;

    let owner_addr = info.sender.clone();

    // Validate pause_admin
    // MUST: only be contract address
    // MUST: not be same address as factory owner (DAO)
    // Any factory action should be done by the owner_addr
    let pause_addr = deps.api.addr_validate(pause_admin.as_str())?;
    if owner_addr == pause_addr || pause_addr.to_string().len() != 63 {
        return Err(ContractError::InvalidPauseAdmin {});
    }

    let config = &Config {
        min_tasks_per_agent: min_tasks_per_agent.unwrap_or(DEFAULT_MIN_TASKS_PER_AGENT),
        croncat_factory_addr: info.sender,
        croncat_manager_key,
        croncat_tasks_key,
        agent_nomination_block_duration: agent_nomination_duration
            .unwrap_or(DEFAULT_NOMINATION_BLOCK_DURATION),
        owner_addr,
        pause_admin,
        agents_eject_threshold: agents_eject_threshold.unwrap_or(DEFAULT_AGENTS_EJECT_THRESHOLD),
        min_coins_for_agent_registration: min_coins_for_agent_registration
            .unwrap_or(DEFAULT_MIN_COINS_FOR_AGENT_REGISTRATION),
        min_active_agent_count: min_active_agent_count.unwrap_or(DEFAULT_MIN_ACTIVE_AGENT_COUNT),
    };

    CONFIG.save(deps.storage, config)?;
    PAUSED.save(deps.storage, &false)?;
    AGENTS_ACTIVE.save(deps.storage, &vec![])?; //Init active agents empty vector
    set_contract_version(
        deps.storage,
        CONTRACT_NAME,
        version.unwrap_or_else(|| CONTRACT_VERSION.to_string()),
    )?;
    AGENT_NOMINATION_STATUS.save(
        deps.storage,
        &AgentNominationStatus {
            start_height_of_nomination: None,
            tasks_created_from_last_nomination: 0,
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", config.owner_addr.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetAgent { account_id } => to_binary(&query_get_agent(deps, env, account_id)?),
        QueryMsg::GetAgentIds { from_index, limit } => {
            to_binary(&query_get_agent_ids(deps, from_index, limit)?)
        }
        QueryMsg::GetAgentTasks { account_id } => {
            to_binary(&query_get_agent_tasks(deps, env, account_id)?)
        }
        QueryMsg::Config {} => to_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::Paused {} => to_binary(&PAUSED.load(deps.storage)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RegisterAgent { payable_account_id } => {
            register_agent(deps, info, env, payable_account_id)
        }
        ExecuteMsg::UnregisterAgent { from_behind } => {
            unregister_agent(deps.storage, &deps.querier, &info.sender, from_behind)
        }
        ExecuteMsg::UpdateAgent { payable_account_id } => {
            update_agent(deps, info, env, payable_account_id)
        }
        ExecuteMsg::CheckInAgent {} => accept_nomination_agent(deps, info, env),
        ExecuteMsg::OnTaskCreated(msg) => on_task_created(env, deps, info, msg),
        ExecuteMsg::OnTaskCompleted(msg) => on_task_completed(deps, info, msg),
        ExecuteMsg::UpdateConfig { config } => execute_update_config(deps, info, config),
        ExecuteMsg::Tick {} => execute_tick(deps, env),
        ExecuteMsg::PauseContract {} => execute_pause(deps, info),
        ExecuteMsg::UnpauseContract {} => execute_unpause(deps, info),
    }
}

fn query_get_agent(deps: Deps, env: Env, account_id: String) -> StdResult<AgentResponse> {
    let account_id = deps.api.addr_validate(&account_id)?;

    let agent = AGENTS.may_load(deps.storage, &account_id)?;

    let a = if let Some(a) = agent {
        a
    } else {
        return Ok(AgentResponse { agent: None });
    };

    let config: Config = CONFIG.load(deps.storage)?;
    let rewards =
        croncat_manager_contract::query_agent_rewards(&deps.querier, &config, account_id.as_str())?;
    let agent_status = get_agent_status(deps.storage, env, &account_id)
        // Return wrapped error if there was a problem
        .map_err(|err| StdError::GenericErr {
            msg: err.to_string(),
        })?;

    let stats = AGENT_STATS
        .may_load(deps.storage, &account_id)?
        .unwrap_or_default();
    let agent_response = AgentResponse {
        agent: Some(AgentInfo {
            status: agent_status,
            payable_account_id: a.payable_account_id,
            balance: rewards,
            last_executed_slot: stats.last_executed_slot,
            register_start: a.register_start,
        }),
    };
    Ok(agent_response)
}

/// Get a list of agent addresses
fn query_get_agent_ids(
    deps: Deps,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<GetAgentIdsResponse> {
    let active_loaded: Vec<Addr> = AGENTS_ACTIVE.load(deps.storage)?;
    let active = active_loaded
        .into_iter()
        .skip(from_index.unwrap_or(DEFAULT_PAGINATION_FROM_INDEX) as usize)
        .take(limit.unwrap_or(DEFAULT_PAGINATION_LIMIT) as usize)
        .collect();
    let pending: Vec<Addr> = AGENTS_PENDING
        .iter(deps.storage)?
        .skip(from_index.unwrap_or(DEFAULT_PAGINATION_FROM_INDEX) as usize)
        .take(limit.unwrap_or(DEFAULT_PAGINATION_LIMIT) as usize)
        .collect::<StdResult<Vec<Addr>>>()?;

    Ok(GetAgentIdsResponse { active, pending })
}

fn query_get_agent_tasks(deps: Deps, env: Env, account_id: String) -> StdResult<AgentTaskResponse> {
    let account_id = deps.api.addr_validate(&account_id)?;
    let active = AGENTS_ACTIVE.load(deps.storage)?;
    if !active.contains(&account_id) {
        return Ok(AgentTaskResponse {
            stats: TaskStats {
                num_cron_tasks: Uint64::zero(),
                num_block_tasks: Uint64::zero(),
            },
        });
    }
    let config: Config = CONFIG.load(deps.storage)?;

    let (block_slots, cron_slots) = croncat_tasks_contract::query_tasks_slots(deps, &config)?;
    if block_slots == 0 && cron_slots == 0 {
        return Ok(AgentTaskResponse {
            stats: TaskStats {
                num_cron_tasks: Uint64::zero(),
                num_block_tasks: Uint64::zero(),
            },
        });
    }
    AGENT_TASK_DISTRIBUTOR
        .get_agent_tasks(
            &deps,
            &env,
            account_id,
            (Some(block_slots), Some(cron_slots)),
        )
        .map_err(|err| StdError::generic_err(err.to_string()))
}

/// Add any account as an agent that will be able to execute tasks.
/// Registering allows for rewards accruing with micro-payments which will accumulate to more long-term.
///
/// Optional Parameters:
/// "payable_account_id" - Allows a different account id to be specified, so a user can receive funds at a different account than the agent account.
fn register_agent(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    payable_account_id: Option<String>,
) -> Result<Response, ContractError> {
    if !info.funds.is_empty() {
        return Err(ContractError::NoFundsShouldBeAttached);
    }
    if PAUSED.load(deps.storage)? {
        return Err(ContractError::ContractPaused);
    }
    let c = CONFIG.load(deps.storage)?;

    let account = info.sender;

    // REF: https://github.com/CosmWasm/cw-tokens/tree/main/contracts/cw20-escrow
    // Check if native token balance is sufficient for a few txns, in this case 4 txns
    let agent_wallet_balances = deps.querier.query_all_balances(account.clone())?;

    // Get the denom from the manager contract
    let manager_config = croncat_manager_contract::query_manager_config(&deps.as_ref(), &c)?;

    let agents_needs_coin = Coin::new(
        c.min_coins_for_agent_registration.into(),
        manager_config.native_denom,
    );
    if !has_coins(&agent_wallet_balances, &agents_needs_coin) || agent_wallet_balances.is_empty() {
        return Err(ContractError::InsufficientFunds {
            amount_needed: agents_needs_coin,
        });
    }

    let payable_id = if let Some(addr) = payable_account_id {
        deps.api.addr_validate(&addr)?
    } else {
        account.clone()
    };

    let mut active_agents_vec: Vec<Addr> = AGENTS_ACTIVE
        .may_load(deps.storage)?
        .ok_or(ContractError::NoActiveAgents)?;
    let total_agents = active_agents_vec.len();
    let agent_status = if total_agents == 0 {
        active_agents_vec.push(account.clone());
        AGENTS_ACTIVE.save(deps.storage, &active_agents_vec)?;
        AgentStatus::Active
    } else {
        AGENTS_PENDING.push_back(deps.storage, &account)?;
        AgentStatus::Pending
    };

    let storage = deps.storage;
    AGENTS.update(
        storage,
        &account,
        |a: Option<Agent>| -> Result<_, ContractError> {
            match a {
                // make sure that account isn't already added
                Some(_) => Err(ContractError::AgentAlreadyRegistered),
                None => {
                    Ok(Agent {
                        payable_account_id: payable_id,
                        // REF: https://github.com/CosmWasm/cosmwasm/blob/main/packages/std/src/types.rs#L57
                        register_start: env.block.time,
                    })
                }
            }
        },
    )?;
    AGENT_STATS.save(
        storage,
        &account,
        &AgentStats {
            last_executed_slot: env.block.height,
            completed_block_tasks: 0,
            completed_cron_tasks: 0,
            missed_blocked_tasks: 0,
            missed_cron_tasks: 0,
        },
    )?;
    Ok(Response::new()
        .add_attribute("action", "register_agent")
        .add_attribute("agent_status", agent_status.to_string()))
}

/// Update agent details, specifically the payable account id for an agent.
fn update_agent(
    deps: DepsMut,
    info: MessageInfo,
    _env: Env,
    payable_account_id: String,
) -> Result<Response, ContractError> {
    let payable_account_id = deps.api.addr_validate(&payable_account_id)?;
    if PAUSED.load(deps.storage)? {
        return Err(ContractError::ContractPaused);
    }

    AGENTS.update(
        deps.storage,
        &info.sender,
        |a: Option<Agent>| -> Result<_, ContractError> {
            match a {
                Some(agent) => {
                    let mut ag = agent;
                    ag.payable_account_id = payable_account_id;
                    Ok(ag)
                }
                None => Err(ContractError::AgentNotRegistered {}),
            }
        },
    )?;

    Ok(Response::new().add_attribute("action", "update_agent"))
}

/// Allows an agent to accept a nomination within a certain amount of time to become an active agent.
fn accept_nomination_agent(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
) -> Result<Response, ContractError> {
    // Compare current time and Config's agent_nomination_begin_time to see if agent can join
    let c: Config = CONFIG.load(deps.storage)?;

    let mut active_agents: Vec<Addr> = AGENTS_ACTIVE.load(deps.storage)?;
    let mut pending_queue_iter = AGENTS_PENDING.iter(deps.storage)?;
    // Agent must be in the pending queue
    // Get the position in the pending queue
    let agent_position = pending_queue_iter
        .position(|a| a.map_or_else(|_| false, |v| info.sender == v))
        .ok_or(ContractError::AgentNotPending)?;
    let agent_nomination_status = AGENT_NOMINATION_STATUS.load(deps.storage)?;
    // edge case if last agent left
    if active_agents.is_empty() && agent_position == 0 {
        active_agents.push(info.sender.clone());
        AGENTS_ACTIVE.save(deps.storage, &active_agents)?;

        AGENTS_PENDING.pop_front(deps.storage)?;
        AGENT_NOMINATION_STATUS.save(
            deps.storage,
            &AgentNominationStatus {
                start_height_of_nomination: None,
                tasks_created_from_last_nomination: 0,
            },
        )?;
        return Ok(Response::new()
            .add_attribute("action", "accept_nomination_agent")
            .add_attribute("new_agent", info.sender.as_str()));
    }

    // It works out such that the time difference between when this is called,
    // and the agent nomination begin time can be divided by the nomination
    // duration and we get an integer. We use that integer to determine if an
    // agent is allowed to get let in. If their position in the pending queue is
    // less than or equal to that integer, they get let in.
    let max_index = max_agent_nomination_index(&c, env, agent_nomination_status)?
        .ok_or(ContractError::TryLaterForNomination)?;
    let kicked_agents = if agent_position as u64 <= max_index {
        // Make this agent active
        // Update state removing from pending queue
        let kicked_agents: Vec<Addr> = {
            let mut kicked = Vec::with_capacity(agent_position);
            for _ in 0..=agent_position {
                let agent = AGENTS_PENDING.pop_front(deps.storage)?;
                // Since we already iterated over it - we know it exists
                let kicked_agent;
                unsafe {
                    kicked_agent = agent.unwrap_unchecked();
                }
                kicked.push(kicked_agent);
            }
            kicked
        };

        // and adding to active queue
        active_agents.push(info.sender.clone());
        AGENTS_ACTIVE.save(deps.storage, &active_agents)?;

        // and update the config, setting the nomination begin time to None,
        // which indicates no one will be nominated until more tasks arrive
        AGENT_NOMINATION_STATUS.save(
            deps.storage,
            &AgentNominationStatus {
                start_height_of_nomination: None,
                tasks_created_from_last_nomination: 0,
            },
        )?;
        kicked_agents
    } else {
        return Err(ContractError::TryLaterForNomination);
    };
    // Find difference
    Ok(Response::new()
        .add_attribute("action", "accept_nomination_agent")
        .add_attribute("new_agent", info.sender.as_str())
        .add_attribute("kicked_agents: ", format!("{kicked_agents:?}")))
}

/// Removes the agent from the active set of AGENTS.
/// Withdraws all reward balances to the agent payable account id.
/// In case it fails to unregister pending agent try to set `from_behind` to true
fn unregister_agent(
    storage: &mut dyn Storage,
    querier: &QuerierWrapper<Empty>,
    agent_id: &Addr,
    from_behind: Option<bool>,
) -> Result<Response, ContractError> {
    if PAUSED.load(storage)? {
        return Err(ContractError::ContractPaused);
    }
    let config: Config = CONFIG.load(storage)?;
    let agent = AGENTS
        .may_load(storage, agent_id)?
        .ok_or(ContractError::AgentNotRegistered {})?;

    // Remove from the list of active agents if the agent in this list
    let mut active_agents: Vec<Addr> = AGENTS_ACTIVE.load(storage)?;
    if let Some(index) = active_agents.iter().position(|addr| addr == agent_id) {
        //Notify the balancer agent has been removed, to rebalance itself
        AGENT_STATS.remove(storage, agent_id);
        active_agents.remove(index);
        AGENTS_ACTIVE.save(storage, &active_agents)?;
    } else {
        // Agent can't be both in active and pending vector
        // Remove from the pending queue
        let mut return_those_agents: Vec<Addr> =
            Vec::with_capacity((AGENTS_PENDING.len(storage)? / 2) as usize);
        if from_behind.unwrap_or(false) {
            while let Some(addr) = AGENTS_PENDING.pop_front(storage)? {
                if addr.eq(agent_id) {
                    break;
                } else {
                    return_those_agents.push(addr);
                }
            }
            for ag in return_those_agents.iter().rev() {
                AGENTS_PENDING.push_front(storage, ag)?;
            }
        } else {
            while let Some(addr) = AGENTS_PENDING.pop_back(storage)? {
                if addr.eq(agent_id) {
                    break;
                } else {
                    return_those_agents.push(addr);
                }
            }
            for ag in return_those_agents.iter().rev() {
                AGENTS_PENDING.push_back(storage, ag)?;
            }
        }
    }
    let msg = croncat_manager_contract::create_withdraw_rewards_submsg(
        querier,
        &config,
        agent_id.as_str(),
        agent.payable_account_id.to_string(),
    )?;
    AGENTS.remove(storage, agent_id);

    let responses = Response::new()
        //Send withdraw rewards message to manager contract
        .add_message(msg)
        .add_attribute("action", "unregister_agent")
        .add_attribute("account_id", agent_id);

    Ok(responses)
}

pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    msg: UpdateConfig,
) -> Result<Response, ContractError> {
    CONFIG.update(deps.storage, |config| {
        // Deconstruct, so we don't miss any fields
        let UpdateConfig {
            croncat_manager_key,
            croncat_tasks_key,
            min_tasks_per_agent,
            agent_nomination_duration,
            min_coins_for_agent_registration,
            agents_eject_threshold,
            min_active_agent_count,
        } = msg;

        validate_config_non_zero_u16(agent_nomination_duration, "agent_nomination_duration")?;
        validate_config_non_zero_u16(min_active_agent_count, "min_active_agent_count")?;
        validate_config_non_zero_u64(min_tasks_per_agent, "min_tasks_per_agent")?;
        validate_config_non_zero_u64(agents_eject_threshold, "agents_eject_threshold")?;
        validate_config_non_zero_u64(
            min_coins_for_agent_registration,
            "min_coins_for_agent_registration",
        )?;

        if info.sender != config.owner_addr {
            return Err(ContractError::Unauthorized {});
        }

        let new_config = Config {
            owner_addr: config.owner_addr,
            pause_admin: config.pause_admin,
            croncat_factory_addr: config.croncat_factory_addr,
            croncat_manager_key: croncat_manager_key.unwrap_or(config.croncat_manager_key),
            croncat_tasks_key: croncat_tasks_key.unwrap_or(config.croncat_tasks_key),
            min_tasks_per_agent: min_tasks_per_agent.unwrap_or(config.min_tasks_per_agent),
            agent_nomination_block_duration: agent_nomination_duration
                .unwrap_or(config.agent_nomination_block_duration),
            min_coins_for_agent_registration: min_coins_for_agent_registration
                .unwrap_or(DEFAULT_MIN_COINS_FOR_AGENT_REGISTRATION),
            agents_eject_threshold: agents_eject_threshold
                .unwrap_or(DEFAULT_AGENTS_EJECT_THRESHOLD),
            min_active_agent_count: min_active_agent_count
                .unwrap_or(DEFAULT_MIN_ACTIVE_AGENT_COUNT),
        };
        Ok(new_config)
    })?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

fn get_agent_status(
    storage: &dyn Storage,
    env: Env,
    account_id: &Addr,
) -> Result<AgentStatus, ContractError> {
    let c: Config = CONFIG.load(storage)?;
    let active = AGENTS_ACTIVE.load(storage)?;

    // Pending
    let mut pending_iter = AGENTS_PENDING.iter(storage)?;
    // If agent is pending, Check if they should get nominated to checkin to become active
    let agent_position = if let Some(pos) = pending_iter.position(|address| {
        if let Ok(addr) = address {
            &addr == account_id
        } else {
            false
        }
    }) {
        pos
    } else {
        // Check for active
        if active.contains(account_id) {
            return Ok(AgentStatus::Active);
        } else {
            return Err(ContractError::AgentNotRegistered {});
        }
    };

    // Edge case if last agent unregistered
    if active.is_empty() && agent_position == 0 {
        return Ok(AgentStatus::Nominated);
    };

    // Load config's task ratio, total tasks, active agents, and AGENT_NOMINATION_BEGIN_TIME.
    // Then determine if this agent is considered "Nominated" and should call CheckInAgent
    let max_agent_index =
        max_agent_nomination_index(&c, env, AGENT_NOMINATION_STATUS.load(storage)?)?;
    let agent_status = match max_agent_index {
        Some(max_idx) if agent_position as u64 <= max_idx => AgentStatus::Nominated,
        _ => AgentStatus::Pending,
    };
    Ok(agent_status)
}

/// Calculate the biggest index of nomination for pending agents
fn max_agent_nomination_index(
    cfg: &Config,
    env: Env,
    agent_nomination_status: AgentNominationStatus,
) -> StdResult<Option<u64>> {
    let block_height = env.block.height;

    let agents_by_tasks_created = agent_nomination_status
        .tasks_created_from_last_nomination
        .saturating_div(cfg.min_tasks_per_agent);
    let agents_by_height = agent_nomination_status
        .start_height_of_nomination
        .map_or(0, |start_height| {
            (block_height - start_height) / cfg.agent_nomination_block_duration as u64
        });
    let agents_to_pass = min(agents_by_tasks_created, agents_by_height);
    if agents_to_pass == 0 {
        Ok(None)
    } else {
        Ok(Some(agents_to_pass - 1))
    }
}

pub fn execute_tick(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let block_height = env.block.height;
    let config = CONFIG.load(deps.storage)?;
    let mut attributes = vec![];
    let mut submessages = vec![];
    let agents_active = AGENTS_ACTIVE.load(deps.storage)?;
    let total_remove_agents: usize = agents_active.len();
    let mut total_removed = 0;

    for agent_id in agents_active {
        let skip = (config.min_active_agent_count as usize) >= total_remove_agents - total_removed;
        if !skip {
            let stats = AGENT_STATS
                .load(deps.storage, &agent_id)
                .unwrap_or_default();
            if block_height > stats.last_executed_slot + config.agents_eject_threshold {
                let resp = unregister_agent(deps.storage, &deps.querier, &agent_id, None)
                    .unwrap_or_default();
                // Save attributes and messages
                attributes.extend_from_slice(&resp.attributes);
                submessages.extend_from_slice(&resp.messages);
                total_removed += 1;
            }
        }
    }

    // Check if there isn't any active or pending agents
    if AGENTS_ACTIVE.load(deps.storage)?.is_empty() && AGENTS_PENDING.is_empty(deps.storage)? {
        attributes.push(Attribute::new("lifecycle", "tick_failure"))
    }
    let response = Response::new()
        .add_attribute("action", "tick")
        .add_attributes(attributes)
        .add_submessages(submessages);
    Ok(response)
}

pub fn execute_pause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    if PAUSED.load(deps.storage)? {
        return Err(ContractError::ContractPaused);
    }
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.pause_admin {
        return Err(ContractError::Unauthorized);
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
        return Err(ContractError::Unauthorized);
    }
    PAUSED.save(deps.storage, &false)?;
    Ok(Response::new().add_attribute("action", "unpause_contract"))
}

fn on_task_created(
    env: Env,
    deps: DepsMut,
    info: MessageInfo,
    _: AgentOnTaskCreated,
) -> Result<Response, ContractError> {
    let config = CONFIG.may_load(deps.storage)?.unwrap();
    croncat_tasks_contract::assert_caller_is_tasks_contract(&deps.querier, &config, &info.sender)?;

    AGENT_NOMINATION_STATUS.update(deps.storage, |mut status| -> StdResult<_> {
        if status.start_height_of_nomination.is_none() {
            status.start_height_of_nomination = Some(env.block.height)
        }
        Ok(AgentNominationStatus {
            start_height_of_nomination: status.start_height_of_nomination,
            tasks_created_from_last_nomination: status.tasks_created_from_last_nomination + 1,
        })
    })?;

    let response = Response::new().add_attribute("action", "on_task_created");
    Ok(response)
}
fn on_task_completed(
    deps: DepsMut,
    info: MessageInfo,
    args: AgentOnTaskCompleted,
) -> Result<Response, ContractError> {
    let config = CONFIG.may_load(deps.storage)?.unwrap();

    croncat_manager_contract::assert_caller_is_manager_contract(
        &deps.querier,
        &config,
        &info.sender,
    )?;
    let mut stats = AGENT_STATS.load(deps.storage, &args.agent_id)?;

    if args.is_block_slot_task {
        stats.completed_block_tasks += 1;
    } else {
        stats.completed_cron_tasks += 1;
    }
    AGENT_STATS.save(deps.storage, &args.agent_id, &stats)?;

    let response = Response::new().add_attribute("action", "on_task_completed");
    Ok(response)
}

/// Validating a non-zero value for u64
fn validate_non_zero(num: u64, field_name: &str) -> Result<(), ContractError> {
    if num == 0u64 {
        Err(InvalidConfigurationValue {
            field: field_name.to_string(),
        })
    } else {
        Ok(())
    }
}

/// Resources indicate that trying to use generics in this case is not the correct path
/// This will cast into a u64 and proceed to validate
fn validate_config_non_zero_u16(
    opt_num: Option<u16>,
    field_name: &str,
) -> Result<(), ContractError> {
    if let Some(num) = opt_num {
        validate_non_zero(num as u64, field_name)
    } else {
        Ok(())
    }
}

fn validate_config_non_zero_u64(
    opt_num: Option<u64>,
    field_name: &str,
) -> Result<(), ContractError> {
    if let Some(num) = opt_num {
        validate_non_zero(num, field_name)
    } else {
        Ok(())
    }
}
