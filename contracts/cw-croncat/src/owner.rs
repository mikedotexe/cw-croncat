use crate::error::ContractError;
use crate::helpers::has_cw_coins;
use crate::state::{Config, CwCroncat};
use cosmwasm_std::{
    has_coins, to_binary, BankMsg, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    SubMsg, WasmMsg,
};
use cw20::{Balance, Cw20ExecuteMsg};
use cw_croncat_core::msg::{
    ExecuteMsg, GetBalancesResponse, GetConfigResponse, GetWalletBalancesResponse,
};
use cw_croncat_core::traits::FindAndMutate;

impl<'a> CwCroncat<'a> {
    pub(crate) fn query_config(&self, deps: Deps) -> StdResult<GetConfigResponse> {
        let c: Config = self.config.load(deps.storage)?;
        Ok(GetConfigResponse {
            paused: c.paused,
            owner_id: c.owner_id,
            // treasury_id: c.treasury_id,
            min_tasks_per_agent: c.min_tasks_per_agent,
            agent_active_indices: c.agent_active_indices,
            agents_eject_threshold: c.agents_eject_threshold,
            native_denom: c.native_denom,
            agent_fee: c.agent_fee,
            gas_fraction: c.gas_fraction,
            proxy_callback_gas: c.proxy_callback_gas,
            slot_granularity_time: c.slot_granularity_time,
            cw_rules_addr: c.cw_rules_addr,
            agent_nomination_duration: c.agent_nomination_duration,
            gas_base_fee: c.gas_base_fee,
            gas_action_fee: c.gas_action_fee,
            cw20_whitelist: c.cw20_whitelist,
            available_balance: c.available_balance,
            staked_balance: c.staked_balance,
            limit: c.limit,
        })
    }

    pub(crate) fn query_balances(&self, deps: Deps) -> StdResult<GetBalancesResponse> {
        let c: Config = self.config.load(deps.storage)?;
        Ok(GetBalancesResponse {
            native_denom: c.native_denom,
            available_balance: c.available_balance,
            staked_balance: c.staked_balance,
            cw20_whitelist: c.cw20_whitelist,
        })
    }

    /// Returns user cw20 balances locked inside this contract
    pub(crate) fn query_wallet_balances(
        &self,
        deps: Deps,
        wallet: String,
    ) -> StdResult<GetWalletBalancesResponse> {
        let addr = deps.api.addr_validate(&wallet)?;
        let balances = self.balances.may_load(deps.storage, &addr)?;
        Ok(GetWalletBalancesResponse {
            cw20_balances: balances.unwrap_or_default(),
        })
    }

    /// Changes core configurations
    /// Should only be updated by owner -- in best case DAO based :)
    pub fn update_settings(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        payload: ExecuteMsg,
    ) -> Result<Response, ContractError> {
        for coin in info.funds.iter() {
            if coin.amount.u128() > 0 {
                return Err(ContractError::AttachedDeposit {});
            }
        }
        let api = deps.api;
        match payload {
            ExecuteMsg::UpdateSettings {
                owner_id,
                slot_granularity_time,
                paused,
                agent_fee,
                gas_base_fee,
                gas_action_fee,
                gas_fraction,
                proxy_callback_gas,
                min_tasks_per_agent,
                agents_eject_threshold,
                // treasury_id,
            } => {
                self.config
                    .update(deps.storage, |mut config| -> Result<_, ContractError> {
                        if info.sender != config.owner_id {
                            return Err(ContractError::Unauthorized {});
                        }

                        if let Some(owner_id) = owner_id {
                            let owner_id = api.addr_validate(&owner_id)?;
                            config.owner_id = owner_id;
                        }
                        // if let Some(treasury_id) = treasury_id {
                        //     config.treasury_id = Some(treasury_id);
                        // }
                        if let Some(slot_granularity_time) = slot_granularity_time {
                            config.slot_granularity_time = slot_granularity_time;
                        }
                        if let Some(paused) = paused {
                            config.paused = paused;
                        }
                        if let Some(gas_base_fee) = gas_base_fee {
                            config.gas_base_fee = gas_base_fee.u64();
                        }
                        if let Some(gas_action_fee) = gas_action_fee {
                            config.gas_action_fee = gas_action_fee.u64();
                        }
                        if let Some(gas_fraction) = gas_fraction {
                            config.gas_fraction = gas_fraction;
                        }
                        if let Some(proxy_callback_gas) = proxy_callback_gas {
                            config.proxy_callback_gas = proxy_callback_gas;
                        }
                        if let Some(agent_fee) = agent_fee {
                            config.agent_fee = agent_fee;
                        }
                        if let Some(min_tasks_per_agent) = min_tasks_per_agent {
                            config.min_tasks_per_agent = min_tasks_per_agent;
                        }
                        if let Some(agents_eject_threshold) = agents_eject_threshold {
                            config.agents_eject_threshold = agents_eject_threshold;
                        }
                        Ok(config)
                    })?;
            }
            _ => unreachable!(),
        }
        let c: Config = self.config.load(deps.storage)?;
        Ok(Response::new()
            .add_attribute("method", "update_settings")
            .add_attribute("paused", c.paused.to_string())
            .add_attribute("owner_id", c.owner_id.to_string())
            // .add_attribute(
            //     "treasury_id",
            //     c.treasury_id
            //         .unwrap_or_else(|| Addr::unchecked(""))
            //         .to_string(),
            // )
            .add_attribute("min_tasks_per_agent", c.min_tasks_per_agent.to_string())
            .add_attribute(
                "agent_active_indices",
                c.agent_active_indices
                    .iter()
                    .map(|a| format!("{:?}.{}", a.0, a.1))
                    .collect::<String>(),
            )
            .add_attribute(
                "agents_eject_threshold",
                c.agents_eject_threshold.to_string(),
            )
            .add_attribute("native_denom", c.native_denom)
            .add_attribute("agent_fee", c.agent_fee.to_string())
            //.add_attribute("gas_price", c.gas_fraction.to_string())
            .add_attribute("proxy_callback_gas", c.proxy_callback_gas.to_string())
            .add_attribute("slot_granularity_time", c.slot_granularity_time.to_string()))
    }

    /// Move Balance
    /// Allows owner to move balance to DAO or to let treasury transfer to itself only.
    /// This is a restricted method for moving funds utilized in growth management strategies.
    pub fn move_balances(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        env: Env,
        balances: Vec<Balance>,
        account_id: String,
    ) -> Result<Response, ContractError> {
        let account_id = deps.api.addr_validate(&account_id)?;
        let mut config = self.config.load(deps.storage)?;

        // // Check if is owner OR the treasury account making the transfer request
        // if let Some(treasury_id) = config.treasury_id.clone() {
        //     if treasury_id != info.sender && config.owner_id != info.sender {
        //         return Err(ContractError::Unauthorized {});
        //     }
        // } else
        if info.sender != config.owner_id {
            return Err(ContractError::Unauthorized {});
        }

        // for now, only allow movement of funds between owner and treasury
        // let check_account = config
        //     .treasury_id
        //     .clone()
        //     .unwrap_or_else(|| config.owner_id.clone());
        let check_account = config.owner_id.clone();
        if check_account != account_id && config.owner_id != account_id {
            return Err(ContractError::CustomError {
                val: "Cannot move funds to this account".to_string(),
            });
        }

        // Querier guarantees to returns up-to-date data, including funds sent in this handle message
        // https://github.com/CosmWasm/wasmd/blob/master/x/wasm/internal/keeper/keeper.go#L185-L192
        let state_balances = deps.querier.query_all_balances(&env.contract.address)?;
        let mut has_fund_err = false;

        let messages: Result<Vec<SubMsg>, ContractError> = balances
            .iter()
            .map(|balance| -> Result<SubMsg<_>, ContractError> {
                match balance {
                    Balance::Native(balance) => {
                        // check has enough
                        let bal = balance.clone().into_vec();
                        let has_c = has_coins(&state_balances, &bal[0]);
                        if !has_c {
                            has_fund_err = true;
                            // TODO: refactor to not need
                            return Ok(SubMsg::new(BankMsg::Send {
                                to_address: account_id.clone().into(),
                                amount: vec![Coin::new(0, "")],
                            }));
                        }

                        // Update internal registry balance
                        config.available_balance.checked_sub_native(&bal)?;
                        Ok(SubMsg::new(BankMsg::Send {
                            to_address: account_id.clone().into(),
                            amount: bal,
                        }))
                    }
                    Balance::Cw20(token) => {
                        // check has enough
                        let bal = token.clone();
                        if !has_cw_coins(&config.available_balance.cw20, &bal) {
                            has_fund_err = true;
                            // TODO: refactor to not need
                            return Ok(SubMsg::new(BankMsg::Send {
                                to_address: account_id.clone().into(),
                                amount: vec![Coin::new(0, "")],
                            }));
                        }

                        // Update internal registry balance
                        config.available_balance.cw20.find_checked_sub(&bal)?;

                        let msg = Cw20ExecuteMsg::Transfer {
                            recipient: account_id.clone().into(),
                            amount: bal.amount,
                        };
                        Ok(SubMsg::new(WasmMsg::Execute {
                            contract_addr: bal.address.to_string(),
                            msg: to_binary(&msg)?,
                            funds: vec![],
                        }))
                    }
                }
            })
            .collect();

        // failed
        if has_fund_err {
            return Err(ContractError::CustomError {
                val: "Not enough funds".to_string(),
            });
        }

        // Update balances in config
        self.config.save(deps.storage, &config)?;

        Ok(Response::new()
            .add_attribute("method", "move_balance")
            .add_attribute("account_id", account_id.to_string())
            .add_submessages(messages?))
    }
}
