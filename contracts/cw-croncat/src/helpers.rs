use crate::state::{Config, QueueItem};
// use cosmwasm_std::Binary;
// use cosmwasm_std::StdError;
// use thiserror::Error;

use crate::tasks::RULE_RES_PLACEHOLDER;
use crate::ContractError::AgentNotRegistered;
use crate::{ContractError, CwCroncat};
use cosmwasm_std::{
    coin, to_binary, Addr, Api, BankMsg, Binary, Coin, CosmosMsg, Env, StdResult, Storage, SubMsg,
    SubMsgResult, WasmMsg,
};
use cw20::{Cw20CoinVerified, Cw20ExecuteMsg};
use cw_croncat_core::msg::ExecuteMsg;
use cw_croncat_core::traits::{BalancesOperations, FindAndMutate};
use cw_croncat_core::types::{calculate_required_amount, Action, AgentStatus};
pub use cw_croncat_core::types::{GenericBalance, Task};
use cw_rules_core::msg::RuleResponse;
//use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::cmp;
use std::ops::Div;
//use std::str::FromStr;
pub(crate) fn vect_difference<T: std::clone::Clone + std::cmp::PartialEq>(
    v1: &[T],
    v2: &[T],
) -> Vec<T> {
    v1.iter().filter(|&x| !v2.contains(x)).cloned().collect()
}

// pub(crate) fn from_raw_str(value: &str) -> Option<Coin> {
//     let re = Regex::new(r"^([0-9.]+)([a-z][a-z0-9]*)$").unwrap();
//     assert!(re.is_match(value));
//     let caps = re.captures(value)?;
//     let amount = caps.get(1).map_or("", |m| m.as_str());
//     let denom = caps.get(2).map_or("", |m| m.as_str());
//     if denom.len() < 3 || denom.len() > 128{
//         return Option::None;
//     }
//     Some(Coin::new(u128::from_str(amount).unwrap(), denom))
// }

// Helper to distribute funds/tokens
pub(crate) fn send_tokens(
    to: &Addr,
    balance: &GenericBalance,
) -> StdResult<(Vec<SubMsg>, GenericBalance)> {
    let native_balance = &balance.native;
    let mut coins: GenericBalance = GenericBalance::default();
    let mut msgs: Vec<SubMsg> = if native_balance.is_empty() {
        vec![]
    } else {
        coins.native = native_balance.to_vec();
        vec![SubMsg::new(BankMsg::Send {
            to_address: to.into(),
            amount: native_balance.to_vec(),
        })]
    };

    let cw20_balance = &balance.cw20;
    let cw20_msgs: StdResult<Vec<_>> = cw20_balance
        .iter()
        .map(|c| {
            let msg = Cw20ExecuteMsg::Transfer {
                recipient: to.into(),
                amount: c.amount,
            };
            let exec = SubMsg::new(WasmMsg::Execute {
                contract_addr: c.address.to_string(),
                msg: to_binary(&msg)?,
                funds: vec![],
            });
            Ok(exec)
        })
        .collect();
    coins.cw20 = cw20_balance.to_vec();
    msgs.append(&mut cw20_msgs?);
    Ok((msgs, coins))
}

/// has_cw_coins returns true if the list of CW20 coins has at least the required amount
pub(crate) fn has_cw_coins(coins: &[Cw20CoinVerified], required: &Cw20CoinVerified) -> bool {
    coins
        .iter()
        .find(|c| c.address == required.address)
        .map(|m| m.amount >= required.amount)
        .unwrap_or(false)
}

pub trait ReplyMsgParser {
    fn transferred_bank_tokens(&self) -> Vec<cosmwasm_std::Coin>;
}

impl ReplyMsgParser for cosmwasm_std::Reply {
    fn transferred_bank_tokens(&self) -> Vec<cosmwasm_std::Coin> {
        if let SubMsgResult::Ok(res) = &self.result {
            res.events
                .iter()
                .filter(|ev| ev.ty == "transfer")
                .flat_map(|ev| {
                    ev.attributes
                        .iter()
                        .filter_map(|attr| {
                            attr.key.eq("amount").then(|| {
                                // I really don't want to put regex here, it's gonna increase binary size way too much
                                let n = attr.value.chars().position(|c| c.is_alphabetic()).unwrap();
                                let (amount, denom) = attr.value.split_at(n);
                                Coin {
                                    amount: amount.parse().unwrap(),
                                    denom: denom.to_owned(),
                                }
                            })
                        })
                        .collect::<Vec<Coin>>()
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl<'a> CwCroncat<'a> {
    pub fn get_agent_status(
        &self,
        storage: &dyn Storage,
        env: Env,
        account_id: Addr,
    ) -> Result<AgentStatus, ContractError> {
        let c: Config = self.config.load(storage)?;
        let block_time = env.block.time.seconds();
        // Check for active
        let active = self.agent_active_queue.load(storage)?;
        if active.contains(&account_id) {
            return Ok(AgentStatus::Active);
        }

        // Pending
        let pending: Vec<Addr> = self.agent_pending_queue.load(storage)?;
        // If agent is pending, Check if they should get nominated to checkin to become active
        let agent_status: AgentStatus = if pending.contains(&account_id) {
            // Load config's task ratio, total tasks, active agents, and agent_nomination_begin_time.
            // Then determine if this agent is considered "Nominated" and should call CheckInAgent
            let min_tasks_per_agent = c.min_tasks_per_agent;
            let total_tasks = self
                .task_total(storage)
                .expect("Unexpected issue getting task total");
            let num_active_agents = self.agent_active_queue.load(storage).unwrap().len() as u64;
            let agent_position = pending
                .iter()
                .position(|address| address == &account_id)
                .unwrap();

            // If we should allow a new agent to take over
            let num_agents_to_accept =
                self.agents_to_let_in(&min_tasks_per_agent, &num_active_agents, &total_tasks);
            let agent_nomination_begin_time = self.agent_nomination_begin_time.load(storage)?;
            match agent_nomination_begin_time {
                Some(begin_time) if num_agents_to_accept > 0 => {
                    let time_difference = block_time - begin_time.seconds();

                    let max_index = cmp::max(
                        time_difference.div(c.agent_nomination_duration as u64),
                        num_agents_to_accept - 1,
                    );
                    if agent_position as u64 <= max_index {
                        AgentStatus::Nominated
                    } else {
                        AgentStatus::Pending
                    }
                }
                _ => {
                    // Not their time yet
                    AgentStatus::Pending
                }
            }
        } else {
            // This should not happen. It means the address is in self.agents
            // but not in the pending or active queues
            // Note: if your IDE highlights the below as problematic, you can ignore
            return Err(AgentNotRegistered {});
        };
        Ok(agent_status)
    }

    pub fn agents_to_let_in(
        &self,
        max_tasks: &u64,
        num_active_agents: &u64,
        total_tasks: &u64,
    ) -> u64 {
        let num_tasks_covered = num_active_agents * max_tasks;
        if total_tasks > &num_tasks_covered {
            // It's possible there are more "covered tasks" than total tasks,
            // so use saturating subtraction to hit zero and not go below
            let total_tasks_needing_agents = total_tasks.saturating_sub(num_tasks_covered);
            let remainder = if total_tasks_needing_agents % max_tasks == 0 {
                0
            } else {
                1
            };
            total_tasks_needing_agents / max_tasks + remainder
        } else {
            0
        }
    }

    // Change balances of task and contract if action did transaction that went through
    pub fn task_after_action(
        &self,
        storage: &mut dyn Storage,
        api: &dyn Api,
        queue_item: QueueItem,
        ok: bool,
    ) -> Result<Task, ContractError> {
        let task_hash = queue_item.task_hash.unwrap();
        let mut task = self.get_task_by_hash(storage, &task_hash)?;
        if ok {
            let mut config = self.config.load(storage)?;
            let action_idx = queue_item.action_idx;
            let action = &task.actions[action_idx as usize];

            // update task balances and contract balances
            if let Some(sent) = action.bank_sent() {
                task.total_deposit.native.checked_sub_coins(sent)?;
                config.available_balance.checked_sub_native(sent)?;
            } else if let Some(sent) = action.cw20_sent(api) {
                task.total_deposit.cw20.find_checked_sub(&sent)?;
                config.available_balance.cw20.find_checked_sub(&sent)?;
            };
            self.config.save(storage, &config)?;
            if task.with_rules() {
                self.tasks_with_rules.save(storage, &task_hash, &task)?;
            } else {
                self.tasks.save(storage, &task_hash, &task)?;
            }
        }
        Ok(task)
    }
}

/// Generate submsgs for this proxy call and the price for it
pub(crate) fn proxy_call_submsgs_price(
    task: &Task,
    cfg: Config,
    next_idx: u64,
) -> Result<(Vec<SubMsg>, Coin), ContractError> {
    let (sub_msgs, gas_total) =
        task.get_submsgs_with_total_gas(cfg.gas_base_fee, cfg.gas_action_fee, next_idx)?;
    let gas_amount = calculate_required_amount(gas_total, cfg.agent_fee)?;
    let price_amount = cfg.gas_fraction.calculate(gas_amount, 1)?;
    let price = coin(price_amount, cfg.native_denom);
    Ok((sub_msgs, price))
}

/// CwTemplateContract is a wrapper around Addr that provides a lot of helpers
/// for working with this.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CwTemplateContract(pub Addr);

impl CwTemplateContract {
    pub fn addr(&self) -> Addr {
        self.0.clone()
    }

    pub fn call<T: Into<ExecuteMsg>>(&self, msg: T) -> StdResult<CosmosMsg> {
        let msg = to_binary(&msg.into())?;
        Ok(WasmMsg::Execute {
            contract_addr: self.addr().into(),
            msg,
            funds: vec![],
        }
        .into())
    }

    // /// Get Count
    // pub fn count<Q, T, CQ>(&self, querier: &Q) -> StdResult<CountResponse>
    // where
    //     Q: Querier,
    //     T: Into<String>,
    //     CQ: CustomQuery,
    // {
    //     let msg = QueryMsg::GetCount {};
    //     let query = WasmQuery::Smart {
    //         contract_addr: self.addr().into(),
    //         msg: to_binary(&msg)?,
    //     }
    //     .into();
    //     let res: CountResponse = QuerierWrapper::<CQ>::new(querier).query(&query)?;
    //     Ok(res)
    // }
}

/// Replace `RULE_RES_PLACEHOLDER` to the result value from the rules
/// Recalculate cw20 usage if any replacements
pub fn replace_placeholders(
    api: &dyn Api,
    cron_addr: &Addr,
    task_hash: &str,
    rules_res: RuleResponse,
    task: Task,
) -> Result<Task, ContractError> {
    if let Some(insertable_data) = rules_res.data {
        let mut task = task;
        let mut replacements_made = false;
        task.actions.iter_mut().for_each(|action| {
            if let CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) = &mut action.msg {
                let position = msg
                    .windows(RULE_RES_PLACEHOLDER.len())
                    .position(|window| window == RULE_RES_PLACEHOLDER);
                if let Some(pos) = position {
                    let mut new_msg = Vec::with_capacity(msg.len() + insertable_data.len());
                    new_msg.extend_from_slice(&msg[..pos]);
                    new_msg.extend_from_slice(insertable_data.as_slice());
                    new_msg.extend_from_slice(&msg[pos + RULE_RES_PLACEHOLDER.len()..]);
                    *msg = Binary::from(new_msg);
                    replacements_made = true;
                }
            }
        });
        if replacements_made {
            let cw20_amount_recalculated =
                calculate_cw20_usage(api, cron_addr, task_hash, &task.actions)?;
            task.amount_for_one_task.cw20 = cw20_amount_recalculated;
            if task
                .verify_enough_cw20(&task.amount_for_one_task.cw20, 1u128.into())
                .is_err()
            {
                return Err(ContractError::TaskNoLongerValid {
                    task_hash: task_hash.to_owned(),
                });
            };
        }
        Ok(task)
    } else {
        Ok(task)
    }
}

fn calculate_cw20_usage(
    api: &dyn Api,
    cron_addr: &Addr,
    task_hash: &str,
    actions: &[Action],
) -> Result<Vec<Cw20CoinVerified>, ContractError> {
    let mut cw20_coins = vec![];
    for action in actions {
        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr, msg, ..
        }) = &action.msg
        {
            if cron_addr.as_str().eq(contract_addr) {
                return Err(ContractError::TaskNoLongerValid {
                    task_hash: task_hash.to_owned(),
                });
            }
            if let Ok(cw20_msg) = cosmwasm_std::from_binary(msg) {
                match cw20_msg {
                    Cw20ExecuteMsg::Send { amount, .. } if !amount.is_zero() => cw20_coins
                        .find_checked_add(&Cw20CoinVerified {
                            address: api.addr_validate(contract_addr)?,
                            amount,
                        })?,
                    Cw20ExecuteMsg::Transfer { amount, .. } if !amount.is_zero() => cw20_coins
                        .find_checked_add(&Cw20CoinVerified {
                            address: api.addr_validate(contract_addr)?,
                            amount,
                        })?,
                    _ => {
                        return Err(ContractError::TaskNoLongerValid {
                            task_hash: task_hash.to_owned(),
                        });
                    }
                }
            }
        }
    }
    Ok(cw20_coins)
}
