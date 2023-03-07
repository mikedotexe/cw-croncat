use crate::error::ContractError;
use cosmwasm_std::{
    to_binary, Addr, CosmosMsg, Deps, Empty, QuerierWrapper, StdError, StdResult, Uint128, WasmMsg,
};
use croncat_sdk_agents::types::Config;
use croncat_sdk_core::internal_messages::agents::AgentWithdrawOnRemovalArgs;
use croncat_sdk_factory::state::CONTRACT_ADDRS;
use croncat_sdk_manager::msg::ManagerQueryMsg;
use croncat_sdk_manager::types::Config as ManagerConfig;
use croncat_sdk_tasks::msg::TasksQueryMsg;
use croncat_sdk_tasks::types::SlotTasksTotalResponse;
pub mod croncat_tasks_contract {
    use super::*;

    pub(crate) fn assert_caller_is_tasks_contract(
        deps_queries: &QuerierWrapper<Empty>,
        config: &Config,
        sender: &Addr,
    ) -> StdResult<()> {
        let addr = query_tasks_addr(deps_queries, config)?;
        if addr != *sender {
            return Err(cosmwasm_std::StdError::GenericErr {
                msg: ContractError::Unauthorized {}.to_string(),
            });
        }
        Ok(())
    }

    pub(crate) fn query_tasks_addr(
        deps_queries: &QuerierWrapper<Empty>,
        config: &Config,
    ) -> StdResult<Addr> {
        let (agents_name, version) = &config.croncat_tasks_key;
        CONTRACT_ADDRS
            .query(
                deps_queries,
                config.croncat_factory_addr.clone(),
                (agents_name, version),
            )?
            .ok_or_else(|| StdError::generic_err(ContractError::InvalidVersionKey {}.to_string()))
    }

    pub fn query_tasks_slots(deps: Deps, config: &Config) -> StdResult<(u64, u64)> {
        let croncat_tasks_addr = query_tasks_addr(&deps.querier, config)?;
        // Get the denom from the manager contract
        let response: SlotTasksTotalResponse = deps.querier.query_wasm_smart(
            croncat_tasks_addr,
            &TasksQueryMsg::SlotTasksTotal { offset: None },
        )?;

        Ok((response.block_tasks, response.cron_tasks))
    }
}

pub mod croncat_manager_contract {

    use super::*;

    pub fn query_manager_config(deps: &Deps, config: &Config) -> StdResult<ManagerConfig> {
        let manager_addr = query_manager_addr(&deps.querier, config)?;
        // Get the denom from the manager contract
        let manager_config: ManagerConfig = deps
            .querier
            .query_wasm_smart(manager_addr, &ManagerQueryMsg::Config {})?;

        Ok(manager_config)
    }

    pub(crate) fn query_manager_addr(
        deps_queries: &QuerierWrapper<Empty>,
        config: &Config,
    ) -> StdResult<Addr> {
        let (manager_name, version) = &config.croncat_manager_key;
        CONTRACT_ADDRS
            .query(
                deps_queries,
                config.croncat_factory_addr.clone(),
                (manager_name, version),
            )?
            .ok_or(cosmwasm_std::StdError::GenericErr {
                msg: ContractError::InvalidVersionKey {}.to_string(),
            })
    }

    pub fn create_withdraw_rewards_submsg(
        querier: &QuerierWrapper<Empty>,
        config: &Config,
        agent_id: &str,
        payable_account_id: String,
    ) -> StdResult<CosmosMsg> {
        let addr = query_manager_addr(querier, config)?;
        let args = AgentWithdrawOnRemovalArgs {
            agent_id: agent_id.to_owned(),
            payable_account_id,
        };
        let execute = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: addr.into(),
            msg: to_binary(&croncat_sdk_manager::msg::ManagerExecuteMsg::AgentWithdraw(
                Some(args),
            ))?,
            funds: vec![],
        });

        Ok(execute)
    }
    pub fn query_agent_rewards(
        querier: &QuerierWrapper<Empty>,
        config: &Config,
        agent_id: &str,
    ) -> StdResult<Uint128> {
        let addr = query_manager_addr(querier, config)?;
        // Get the denom from the manager contract
        let response: Uint128 = querier.query_wasm_smart(
            addr,
            &ManagerQueryMsg::AgentRewards {
                agent_id: agent_id.to_owned(),
            },
        )?;

        Ok(response)
    }
    pub(crate) fn assert_caller_is_manager_contract(
        deps_queries: &QuerierWrapper<Empty>,
        config: &Config,
        sender: &Addr,
    ) -> StdResult<()> {
        let addr = query_manager_addr(deps_queries, config)?;
        if addr != *sender {
            return Err(cosmwasm_std::StdError::GenericErr {
                msg: ContractError::Unauthorized {}.to_string(),
            });
        }
        Ok(())
    }
}
