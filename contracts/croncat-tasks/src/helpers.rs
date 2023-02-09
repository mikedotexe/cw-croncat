use cosmwasm_std::{
    Addr, BankMsg, Binary, BlockInfo, CosmosMsg, Deps, Empty, QuerierWrapper, StdResult, Storage,
    WasmMsg,
};
use croncat_sdk_tasks::types::{
    AmountForOneTask, Boundary, BoundaryHeight, BoundaryTime, Config, Interval, TaskRequest,
};
use cw20::{Cw20CoinVerified, Cw20ExecuteMsg};

use crate::{
    state::{tasks_map, BLOCK_SLOTS, EVENTED_TASKS_LOOKUP, TASKS_TOTAL, TASK_SLOT, TIME_SLOTS},
    ContractError,
};

pub(crate) fn validate_boundary(
    block_info: &BlockInfo,
    boundary: Option<Boundary>,
    interval: &Interval,
) -> Result<Boundary, ContractError> {
    match (interval, boundary) {
        (Interval::Cron(_), Some(Boundary::Time(boundary_time))) => {
            let starting_time = boundary_time.start.unwrap_or(block_info.time);
            if boundary_time.end.map_or(false, |e| e <= starting_time) {
                Err(ContractError::InvalidBoundary {})
            } else {
                Ok(Boundary::Time(boundary_time))
            }
        }
        (
            Interval::Block(_) | Interval::Once | Interval::Immediate,
            Some(Boundary::Height(boundary_height)),
        ) => {
            let starting_height = boundary_height
                .start
                .map(Into::into)
                .unwrap_or(block_info.height);
            if boundary_height
                .end
                .map_or(false, |e| e.u64() <= starting_height)
            {
                Err(ContractError::InvalidBoundary {})
            } else {
                Ok(Boundary::Height(boundary_height))
            }
        }
        (Interval::Cron(_), None) => Ok(Boundary::Time(BoundaryTime {
            start: None,
            end: None,
        })),
        (_, None) => Ok(Boundary::Height(BoundaryHeight {
            start: None,
            end: None,
        })),
        _ => Err(ContractError::InvalidBoundary {}),
    }
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
        return Err(ContractError::InvalidAction {});
    } else if contract_addr == manager_addr {
        // Check if caller is manager owner
        if sender != manager_owner_addr {
            return Err(ContractError::InvalidAction {});
        } else if let Ok(msg) = cosmwasm_std::from_binary(msg) {
            // Check if it's tick
            if !matches!(msg, croncat_sdk_agents::msg::ExecuteMsg::Tick {}) {
                return Err(ContractError::InvalidAction {});
            }
            // Other messages not allowed
        } else {
            return Err(ContractError::InvalidAction {});
        }
    }
    Ok(())
}

pub(crate) fn validate_msg_calculate_usage(
    deps: Deps,
    task: &TaskRequest,
    self_addr: &Addr,
    sender: &Addr,
    config: &Config,
) -> Result<AmountForOneTask, ContractError> {
    let mut amount_for_one_task = AmountForOneTask {
        gas: config.gas_base_fee,
        cw20: None,
        coin: [None, None],
    };

    let manager_addr = get_manager_addr(&deps.querier, config)?;
    let agents_addr = get_agents_addr(&deps.querier, config)?;

    let manager_conf: croncat_sdk_manager::types::Config = deps.querier.query_wasm_smart(
        &manager_addr,
        &croncat_sdk_manager::msg::ManagerQueryMsg::Config {},
    )?;

    if task.actions.is_empty() {
        return Err(ContractError::InvalidAction {});
    }
    for action in task.actions.iter() {
        amount_for_one_task.add_gas(action.gas_limit.unwrap_or(config.gas_action_fee));

        match &action.msg {
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr,
                funds,
                msg,
            }) => {
                if action.gas_limit.is_none() {
                    return Err(ContractError::NoGasLimit {});
                }
                for coin in funds {
                    if coin.amount.is_zero() || !amount_for_one_task.add_coin(coin.clone())? {
                        return Err(ContractError::InvalidAction {});
                    }
                }
                check_for_self_calls(
                    self_addr,
                    &manager_addr,
                    &agents_addr,
                    &manager_conf.owner_addr,
                    sender,
                    contract_addr,
                    msg,
                )?;
                if let Ok(cw20_msg) = cosmwasm_std::from_binary(msg) {
                    match cw20_msg {
                        Cw20ExecuteMsg::Send { amount, .. } if !amount.is_zero() => {
                            if !amount_for_one_task.add_cw20(Cw20CoinVerified {
                                address: deps.api.addr_validate(contract_addr)?,
                                amount,
                            }) {
                                return Err(ContractError::InvalidAction {});
                            }
                        }
                        Cw20ExecuteMsg::Transfer { amount, .. } if !amount.is_zero() => {
                            if !amount_for_one_task.add_cw20(Cw20CoinVerified {
                                address: deps.api.addr_validate(contract_addr)?,
                                amount,
                            }) {
                                return Err(ContractError::InvalidAction {});
                            }
                        }
                        _ => {
                            return Err(ContractError::InvalidAction {});
                        }
                    }
                }
            }
            CosmosMsg::Bank(BankMsg::Send {
                to_address: _,
                amount,
            }) => {
                // Restrict no-coin transfer
                if amount.is_empty() {
                    return Err(ContractError::InvalidAction {});
                }
                for coin in amount {
                    // Zero coins will fail the transaction
                    if coin.amount.is_zero() || !amount_for_one_task.add_coin(coin.clone())? {
                        return Err(ContractError::InvalidAction {});
                    }
                }
            }
            // Disallow unknown messages
            _ => {
                return Err(ContractError::InvalidAction {});
            }
        }
    }

    if let Some(queries) = &task.queries {
        amount_for_one_task.add_gas(queries.len() as u64 * config.gas_query_fee)
    }
    Ok(amount_for_one_task)
}

pub(crate) fn remove_task(
    storage: &mut dyn Storage,
    hash: &[u8],
    is_block: bool,
    is_evented: bool,
) -> StdResult<()> {
    tasks_map().remove(storage, hash)?;
    TASKS_TOTAL.update(storage, |total| StdResult::Ok(total - 1))?;
    let slot_id = TASK_SLOT.load(storage, hash)?;

    if is_evented {
        EVENTED_TASKS_LOOKUP.remove(storage, (slot_id, hash));
    } else if is_block {
        BLOCK_SLOTS.remove(storage, (slot_id, hash));
    } else {
        TIME_SLOTS.remove(storage, (slot_id, hash));
    }
    TASK_SLOT.remove(storage, hash);

    Ok(())
}

pub(crate) fn check_if_sender_is_manager(
    deps_queries: &QuerierWrapper<Empty>,
    config: &Config,
    sender: &Addr,
) -> Result<(), ContractError> {
    let manager_addr = get_manager_addr(deps_queries, config)?;
    if manager_addr != *sender {
        return Err(ContractError::Unauthorized {});
    }

    Ok(())
}

pub(crate) fn get_manager_addr(
    deps_queries: &QuerierWrapper<Empty>,
    config: &Config,
) -> Result<Addr, ContractError> {
    let (manager_name, version) = &config.croncat_manager_key;
    croncat_sdk_factory::state::CONTRACT_ADDRS
        .query(
            deps_queries,
            config.croncat_factory_addr.clone(),
            (manager_name, version),
        )?
        .ok_or(ContractError::InvalidKey {})
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

#[cfg(test)]
mod tests {
    use cosmwasm_std::{Timestamp, Uint64};

    use super::*;

    #[test]
    fn validate_boundary_cases() {
        type ValidateBoundaryChecker = (
            Interval,
            Option<Boundary>,
            // current block height
            u64,
            // current block timestamp
            Timestamp,
            // expected result
            Result<Boundary, ContractError>,
        );
        let cases: Vec<ValidateBoundaryChecker> = vec![
            // Boundary - None
            (
                Interval::Once,
                None,
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: None,
                    end: None,
                })),
            ),
            (
                Interval::Immediate,
                None,
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: None,
                    end: None,
                })),
            ),
            (
                Interval::Block(5),
                None,
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: None,
                    end: None,
                })),
            ),
            (
                Interval::Cron("* * * * * *".to_owned()),
                None,
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Time(BoundaryTime {
                    start: None,
                    end: None,
                })),
            ),
            // Boundary height, start&end - None
            (
                Interval::Once,
                Some(Boundary::Height(BoundaryHeight {
                    start: None,
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: None,
                    end: None,
                })),
            ),
            (
                Interval::Immediate,
                Some(Boundary::Height(BoundaryHeight {
                    start: None,
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: None,
                    end: None,
                })),
            ),
            (
                Interval::Block(5),
                Some(Boundary::Height(BoundaryHeight {
                    start: None,
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: None,
                    end: None,
                })),
            ),
            (
                Interval::Cron("* * * * * *".to_owned()),
                Some(Boundary::Height(BoundaryHeight {
                    start: None,
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Err(ContractError::InvalidBoundary {}),
            ),
            // Boundary Time - start&end - None
            (
                Interval::Once,
                Some(Boundary::Time(BoundaryTime {
                    start: None,
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Err(ContractError::InvalidBoundary {}),
            ),
            (
                Interval::Immediate,
                Some(Boundary::Time(BoundaryTime {
                    start: None,
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Err(ContractError::InvalidBoundary {}),
            ),
            (
                Interval::Block(5),
                Some(Boundary::Time(BoundaryTime {
                    start: None,
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Err(ContractError::InvalidBoundary {}),
            ),
            (
                Interval::Cron("* * * * * *".to_owned()),
                Some(Boundary::Time(BoundaryTime {
                    start: None,
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Time(BoundaryTime {
                    start: None,
                    end: None,
                })),
            ),
            // Start exactly now
            (
                Interval::Once,
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: None,
                })),
            ),
            (
                Interval::Immediate,
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: None,
                })),
            ),
            (
                Interval::Block(5),
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: None,
                })),
            ),
            (
                Interval::Cron("* * * * * *".to_owned()),
                Some(Boundary::Time(BoundaryTime {
                    start: Some(Timestamp::from_nanos(123456)),
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Time(BoundaryTime {
                    start: Some(Timestamp::from_nanos(123456)),
                    end: None,
                })),
            ),
            // Start 1 too early, we shouldn't check it
            (
                Interval::Once,
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(122)),
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(122)),
                    end: None,
                })),
            ),
            (
                Interval::Immediate,
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(122)),
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(122)),
                    end: None,
                })),
            ),
            (
                Interval::Block(5),
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(122)),
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(122)),
                    end: None,
                })),
            ),
            (
                Interval::Cron("* * * * * *".to_owned()),
                Some(Boundary::Time(BoundaryTime {
                    start: Some(Timestamp::from_nanos(123455)),
                    end: None,
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Time(BoundaryTime {
                    start: Some(Timestamp::from_nanos(123455)),
                    end: None,
                })),
            ),
            // Ok ends
            (
                Interval::Once,
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: Some(Uint64::new(124)),
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: Some(Uint64::new(124)),
                })),
            ),
            (
                Interval::Immediate,
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: Some(Uint64::new(124)),
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: Some(Uint64::new(124)),
                })),
            ),
            (
                Interval::Block(5),
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: Some(Uint64::new(124)),
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: Some(Uint64::new(124)),
                })),
            ),
            (
                Interval::Cron("* * * * * *".to_owned()),
                Some(Boundary::Time(BoundaryTime {
                    start: Some(Timestamp::from_nanos(123456)),
                    end: Some(Timestamp::from_nanos(123457)),
                })),
                123,
                Timestamp::from_nanos(123456),
                Ok(Boundary::Time(BoundaryTime {
                    start: Some(Timestamp::from_nanos(123456)),
                    end: Some(Timestamp::from_nanos(123457)),
                })),
            ),
            // End too early
            (
                Interval::Once,
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: Some(Uint64::new(123)),
                })),
                123,
                Timestamp::from_nanos(123456),
                Err(ContractError::InvalidBoundary {}),
            ),
            (
                Interval::Immediate,
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: Some(Uint64::new(123)),
                })),
                123,
                Timestamp::from_nanos(123456),
                Err(ContractError::InvalidBoundary {}),
            ),
            (
                Interval::Block(5),
                Some(Boundary::Height(BoundaryHeight {
                    start: Some(Uint64::new(123)),
                    end: Some(Uint64::new(123)),
                })),
                123,
                Timestamp::from_nanos(123456),
                Err(ContractError::InvalidBoundary {}),
            ),
            (
                Interval::Cron("* * * * * *".to_owned()),
                Some(Boundary::Time(BoundaryTime {
                    start: Some(Timestamp::from_nanos(123456)),
                    end: Some(Timestamp::from_nanos(123456)),
                })),
                123,
                Timestamp::from_nanos(123456),
                Err(ContractError::InvalidBoundary {}),
            ),
        ];
        for (interval, boundary, height, time, expected_res) in cases {
            let block_info = BlockInfo {
                height,
                time,
                chain_id: "cron".to_owned(),
            };
            let res = validate_boundary(&block_info, boundary, &interval);
            assert_eq!(res, expected_res)
        }
    }
}
