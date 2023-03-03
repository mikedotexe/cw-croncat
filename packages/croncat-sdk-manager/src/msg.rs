use crate::types::UpdateConfig;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use croncat_sdk_core::internal_messages::agents::AgentWithdrawOnRemovalArgs;
use croncat_sdk_core::internal_messages::manager::{ManagerCreateTaskBalance, ManagerRemoveTask};
use croncat_sdk_core::types::GasPrice;

use cw20::Cw20Coin;

#[cw_serde]
pub struct ManagerInstantiateMsg {
    /// CW2 Version provided by factory
    pub version: Option<String>,
    /// Name of the key for raw querying Tasks address from the factory
    pub croncat_tasks_key: (String, [u8; 2]),
    /// Name of the key for raw querying Agents address from the factory
    pub croncat_agents_key: (String, [u8; 2]),
    /// A multisig admin whose sole responsibility is to pause the contract in event of emergency.
    /// Must be a different contract address than DAO, cannot be a regular keypair
    /// Does not have the ability to unpause, must rely on the DAO to assess the situation and act accordingly
    pub pause_admin: Addr,
    /// Gas prices that expected to be used by the agent
    pub gas_price: Option<GasPrice>,

    /// Contract's treasury.
    /// Fees from tasks will go to this address, if set or to the owner address otherwise
    pub treasury_addr: Option<String>,

    /// List of whitelisted cw20s
    pub cw20_whitelist: Option<Vec<String>>,
}

#[cw_serde]
pub enum ManagerExecuteMsg {
    /// Updates the croncat Config.
    /// Note: it's shared across contracts
    // Boxing cause of large enum variant
    UpdateConfig(Box<UpdateConfig>),

    /// Execute current task in the queue or task with queries if task_hash given
    ProxyCall {
        task_hash: Option<String>,
    },

    /// Receive native coins to include them to the task
    RefillTaskBalance {
        task_hash: String,
    },
    RefillTaskCw20Balance {
        task_hash: String,
        cw20: Cw20Coin,
    },

    /// Receive cw20 coin
    Receive(cw20::Cw20ReceiveMsg),

    /// Create task's balance, called by the tasks contract
    CreateTaskBalance(Box<ManagerCreateTaskBalance>),

    /// Remove task's balance, called by the tasks contract
    RemoveTask(ManagerRemoveTask),

    /// Move balances from the manager to the owner address, or treasury_addr if set
    OwnerWithdraw {},

    /// Withdraw temp coins for users
    UserWithdraw {
        // In case user somehow manages to have too many coins we don't want them to get locked funds
        limit: Option<u64>,
    },

    /// Withdraw agent rewards on agent removal, this should be called only by agent contract
    AgentWithdraw(Option<AgentWithdrawOnRemovalArgs>),

    /// Pauses all operations for this contract, can only be done by pause_admin
    PauseContract {},
    /// unpauses all operations for this contract, can only be unpaused by owner_addr
    UnpauseContract {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum ManagerQueryMsg {
    /// Gets current croncat config
    #[returns(crate::types::Config)]
    Config {},

    /// Helper for query responses on versioned contracts
    #[returns[bool]]
    Paused {},

    /// Gets manager available balances
    #[returns(cosmwasm_std::Uint128)]
    TreasuryBalance {},
    /// Gets Cw20 balances of the given wallet address
    #[returns(Vec<cw20::Cw20CoinVerified>)]
    UsersBalances {
        address: String,
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    /// Get task balance
    #[returns(crate::types::TaskBalanceResponse)]
    TaskBalance { task_hash: String },

    #[returns(cosmwasm_std::Uint128)]
    AgentRewards { agent_id: String },
}

#[cw_serde]
pub enum ManagerReceiveMsg {
    RefillTempBalance {},
    RefillTaskBalance { task_hash: String },
}
#[cw_serde]
pub struct AgentWithdrawCallback {
    pub agent_id: String,
    pub amount: Uint128,
    pub payable_account_id: String,
}
