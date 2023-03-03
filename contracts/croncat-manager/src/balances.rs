use cosmwasm_std::{
    coins, from_binary, to_binary, Addr, BankMsg, Deps, DepsMut, MessageInfo, Order, Response,
    StdError, StdResult, Storage, Uint128, WasmMsg,
};
use croncat_sdk_core::types::GasPrice;
use croncat_sdk_manager::types::Config;
use cw20::{Cw20Coin, Cw20CoinVerified, Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::{
    helpers::{check_if_sender_is_task_owner, check_ready_for_execution, gas_fee, get_tasks_addr},
    msg::ReceiveMsg,
    state::{AGENT_REWARDS, CONFIG, PAUSED, TASKS_BALANCES, TEMP_BALANCES_CW20, TREASURY_BALANCE},
    ContractError,
};

pub(crate) fn add_user_cw20(
    storage: &mut dyn Storage,
    user_addr: &Addr,
    cw20: &Cw20CoinVerified,
) -> StdResult<Uint128> {
    let new_bal = TEMP_BALANCES_CW20.update(
        storage,
        (user_addr, &cw20.address),
        |bal| -> StdResult<Uint128> {
            let bal = bal.unwrap_or_default();
            Ok(bal.checked_add(cw20.amount)?)
        },
    )?;
    Ok(new_bal)
}

pub(crate) fn sub_user_cw20(
    storage: &mut dyn Storage,
    user_addr: &Addr,
    cw20: &Cw20CoinVerified,
) -> Result<Uint128, ContractError> {
    let current_balance = TEMP_BALANCES_CW20.may_load(storage, (user_addr, &cw20.address))?;
    let new_bal = if let Some(bal) = current_balance {
        bal.checked_sub(cw20.amount).map_err(StdError::overflow)?
    } else {
        return Err(ContractError::EmptyBalance {});
    };

    if new_bal.is_zero() {
        TEMP_BALANCES_CW20.remove(storage, (user_addr, &cw20.address));
    } else {
        TEMP_BALANCES_CW20.save(storage, (user_addr, &cw20.address), &new_bal)?;
    }
    Ok(new_bal)
}

/// Adding agent and treasury rewards
/// Refunding gas used by the agent for this task
/// For example, if we have both `agent_fee`&`treasury_fee` set at 5% :
/// 105% of gas cost goes to the agents (100% to cover gas used for this transaction and 5% as a reward)
/// and remaining 5% goes to the treasury
pub(crate) fn add_fee_rewards(
    storage: &mut dyn Storage,
    gas: u64,
    gas_price: &GasPrice,
    agent_addr: &Addr,
    agent_fee: u16,
    treasury_fee: u16,
    reimburse_only: bool,
) -> Result<(), ContractError> {
    AGENT_REWARDS.update(
        storage,
        agent_addr,
        |agent_balance| -> Result<_, ContractError> {
            // Adding base gas and agent_fee here
            let gas_fee = if reimburse_only {
                gas
            } else {
                gas_fee(gas, agent_fee.into())? + gas
            };
            let amount: Uint128 = gas_price.calculate(gas_fee).unwrap().into();
            Ok(agent_balance.unwrap_or_default().saturating_add(amount))
        },
    )?;

    if !reimburse_only {
        TREASURY_BALANCE.update(storage, |balance| -> Result<_, ContractError> {
            let gas_fee = gas_fee(gas, treasury_fee.into())?;
            let amount: Uint128 = gas_price.calculate(gas_fee).unwrap().into();
            Ok(balance.saturating_add(amount))
        })?;
    }

    Ok(())
}

// Contract methods

/// Execute: Receive
/// Message validated, to be sure about intention of transferred tokens
/// Used by users before creating a task with cw20 send or transfer messages
///
/// Returns updated balances
pub fn execute_receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let msg: ReceiveMsg = from_binary(&wrapper.msg)?;
    let paused = PAUSED.load(deps.storage)?;
    check_ready_for_execution(&info, paused)?;
    let config = CONFIG.load(deps.storage)?;

    let sender = deps.api.addr_validate(&wrapper.sender)?;
    let coin_addr = info.sender;
    if !config.cw20_whitelist.contains(&coin_addr) {
        return Err(ContractError::NotSupportedCw20 {});
    }

    let cw20_verified = Cw20CoinVerified {
        address: coin_addr,
        amount: wrapper.amount,
    };

    match msg {
        ReceiveMsg::RefillTempBalance {} => {
            let user_cw20_balance = add_user_cw20(deps.storage, &sender, &cw20_verified)?;
            Ok(Response::new()
                .add_attribute("action", "receive_cw20")
                .add_attribute("cw20_received", cw20_verified.to_string())
                .add_attribute("user_cw20_balance", user_cw20_balance))
        }
        ReceiveMsg::RefillTaskBalance { task_hash } => {
            // Check if sender is task owner
            let tasks_addr = get_tasks_addr(&deps.querier, &config)?;
            check_if_sender_is_task_owner(&deps.querier, &tasks_addr, &sender, &task_hash)?;

            let mut task_balances = TASKS_BALANCES
                .may_load(deps.storage, task_hash.as_bytes())?
                .ok_or(ContractError::NoTaskHash {})?;
            let mut balance = task_balances
                .cw20_balance
                .ok_or(ContractError::InvalidAttachedCoins {})?;
            if balance.address != cw20_verified.address {
                return Err(ContractError::InvalidAttachedCoins {});
            }
            balance.amount += cw20_verified.amount;
            task_balances.cw20_balance = Some(balance);
            TASKS_BALANCES.save(deps.storage, task_hash.as_bytes(), &task_balances)?;
            Ok(Response::new()
                .add_attribute("action", "receive_cw20")
                .add_attribute("cw20_received", cw20_verified.to_string())
                .add_attribute(
                    "task_cw20_balance",
                    task_balances.cw20_balance.unwrap().amount,
                ))
        }
    }
}

pub fn execute_refill_task_cw20(
    deps: DepsMut,
    info: MessageInfo,
    task_hash: String,
    cw20: Cw20Coin,
) -> Result<Response, ContractError> {
    let paused = PAUSED.load(deps.storage)?;
    check_ready_for_execution(&info, paused)?;
    let config = CONFIG.load(deps.storage)?;

    // check if sender is task owner
    let tasks_addr = get_tasks_addr(&deps.querier, &config)?;
    check_if_sender_is_task_owner(&deps.querier, &tasks_addr, &info.sender, &task_hash)?;

    let cw20_verified = Cw20CoinVerified {
        address: deps.api.addr_validate(&cw20.address)?,
        amount: cw20.amount,
    };

    sub_user_cw20(deps.storage, &info.sender, &cw20_verified)?;
    let mut task_balances = TASKS_BALANCES
        .may_load(deps.storage, task_hash.as_bytes())?
        .ok_or(ContractError::NoTaskHash {})?;
    let mut balance = task_balances
        .cw20_balance
        .ok_or(ContractError::InvalidAttachedCoins {})?;
    if balance.address != cw20_verified.address {
        return Err(ContractError::InvalidAttachedCoins {});
    }
    balance.amount += cw20_verified.amount;
    task_balances.cw20_balance = Some(balance);
    TASKS_BALANCES.save(deps.storage, task_hash.as_bytes(), &task_balances)?;

    Ok(Response::new()
        .add_attribute("action", "refill_task_cw20")
        .add_attribute("cw20_refilled", cw20_verified.to_string())
        .add_attribute(
            "task_cw20_balance",
            task_balances.cw20_balance.unwrap().to_string(),
        ))
}

/// Execute: WithdrawCw20WalletBalances
/// Used by users to withdraw back their cw20 tokens
///
/// Returns updated balances
///
/// NOTE: During paused configuration, all funds will be temporarily locked.
/// This is currently to safeguard all execution paths. All funds (not just user funds)
/// are locked, until any pause concern has been addressed or finished. In many cases,
/// this will occur for simple contract upgrades, but could be caused from DAO identified
/// security risks. The pause-lock will be removed as future contract testing proves
/// mature enough, deemed ready by DAO. We expect this to be several months post-launch.
pub fn execute_user_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    limit: Option<u64>,
) -> Result<Response, ContractError> {
    let paused = PAUSED.load(deps.storage)?;
    check_ready_for_execution(&info, paused)?;
    let config = CONFIG.load(deps.storage)?;
    let limit = limit.unwrap_or(config.limit);
    let user_addr = info.sender;
    let withdraws: Vec<Cw20CoinVerified> = TEMP_BALANCES_CW20
        .prefix(&user_addr)
        .range(deps.storage, None, None, Order::Ascending)
        .take(limit as usize)
        .map(|cw20_res| cw20_res.map(|(address, amount)| Cw20CoinVerified { address, amount }))
        .collect::<StdResult<_>>()?;
    if withdraws.is_empty() {
        return Err(ContractError::EmptyBalance {});
    }
    // update user and croncat manager balances
    for cw20 in withdraws.iter() {
        sub_user_cw20(deps.storage, &user_addr, cw20)?;
    }

    let msgs = {
        let mut msgs = Vec::with_capacity(withdraws.len());
        for wd in withdraws {
            msgs.push(WasmMsg::Execute {
                contract_addr: wd.address.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: user_addr.to_string(),
                    amount: wd.amount,
                })?,
                funds: vec![],
            });
        }
        msgs
    };

    Ok(Response::new()
        .add_attribute("action", "user_withdraw")
        .add_messages(msgs))
}

/// Execute: OwnerWithdraw
/// Used by owner of the contract to move balances from the manager to treasury or owner address
pub fn execute_owner_withdraw(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner_addr {
        return Err(ContractError::Unauthorized {});
    }
    let address = config.treasury_addr.unwrap_or(config.owner_addr);

    let withdraw = TREASURY_BALANCE.load(deps.storage)?;
    TREASURY_BALANCE.save(deps.storage, &Uint128::zero())?;

    if withdraw.is_zero() {
        Err(ContractError::EmptyBalance {})
    } else {
        let bank_msg = BankMsg::Send {
            to_address: address.into_string(),
            amount: coins(withdraw.u128(), config.native_denom),
        };
        Ok(Response::new()
            .add_attribute("action", "owner_withdraw")
            .add_message(bank_msg))
    }
}

pub fn execute_refill_native_balance(
    deps: DepsMut,
    info: MessageInfo,
    task_hash: String,
) -> Result<Response, ContractError> {
    if PAUSED.load(deps.storage)? {
        return Err(ContractError::ContractPaused {});
    }
    let config: Config = CONFIG.load(deps.storage)?;
    // Check if sender is task owner
    let tasks_addr = get_tasks_addr(&deps.querier, &config)?;
    check_if_sender_is_task_owner(&deps.querier, &tasks_addr, &info.sender, &task_hash)?;

    let mut task_balances = TASKS_BALANCES
        .may_load(deps.storage, task_hash.as_bytes())?
        .ok_or(ContractError::NoTaskHash {})?;

    if info.funds.len() > 2 {
        return Err(ContractError::InvalidAttachedCoins {});
    }
    for coin in info.funds {
        if coin.denom == config.native_denom {
            task_balances.native_balance += coin.amount
        } else {
            let mut ibc = task_balances
                .ibc_balance
                .ok_or(ContractError::InvalidAttachedCoins {})?;
            if ibc.denom != coin.denom {
                return Err(ContractError::InvalidAttachedCoins {});
            }
            ibc.amount += coin.amount;
            task_balances.ibc_balance = Some(ibc);
        }
    }
    TASKS_BALANCES.save(deps.storage, task_hash.as_bytes(), &task_balances)?;
    Ok(Response::new().add_attribute("action", "refill_native_balance"))
}

/// Query: Cw20WalletBalances
/// Used to get user's available cw20 coins balance that he can use to attach to the task balance
/// Can be paginated
///
/// Returns list of cw20 balances
pub fn query_users_balances(
    deps: Deps,
    address: String,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<Cw20CoinVerified>> {
    let config = CONFIG.load(deps.storage)?;
    let addr = deps.api.addr_validate(&address)?;
    let from_index = from_index.unwrap_or_default();
    let limit = limit.unwrap_or(config.limit);

    let cw20_balance = TEMP_BALANCES_CW20
        .prefix(&addr)
        .range(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .map(|balance_res| {
            balance_res.map(|(addr, amount)| Cw20CoinVerified {
                address: addr,
                amount,
            })
        })
        .collect::<StdResult<_>>()?;

    Ok(cw20_balance)
}
