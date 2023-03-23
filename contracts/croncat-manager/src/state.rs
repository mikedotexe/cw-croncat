use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use croncat_sdk_manager::types::TaskBalance;
use cw_storage_plus::{Item, Map};

pub use croncat_sdk_manager::types::Config;
use croncat_sdk_tasks::types::TaskExecutionInfo;

pub const CONFIG: Item<Config> = Item::new("config");

/// Controls whether or not the contract is paused. Can only be changed to TRUE by
/// the pause_admin, can only be unpaused by DAO/owner_addr
pub const PAUSED: Item<bool> = Item::new("paused");

// Accrued Treasury reward balance in native coin
pub const TREASURY_BALANCE: Item<Uint128> = Item::new("treasury_balance");

// Accrued Agent reward balance in native coin
pub const AGENT_REWARDS: Map<&Addr, Uint128> = Map::new("agent_rewards");

// Temporary balances of users before task creation.
// Please do not store your coins for any other use.
pub const TEMP_BALANCES_CW20: Map<(&Addr, &Addr), Uint128> = Map::new("temp_balances_cw20");

pub const TASKS_BALANCES: Map<&[u8], TaskBalance> = Map::new("tasks_balances");

pub const REPLY_QUEUE: Item<QueueItem> = Item::new("reply_queue");

pub const LAST_TASK_EXECUTION_INFO: Item<TaskExecutionInfo> = Item::new("last_task_execution_info");

/// This struct will keep the task and who is doing it until the last action
#[cw_serde]
pub struct QueueItem {
    pub task: croncat_sdk_tasks::types::TaskInfo,
    pub agent_addr: Addr,
    /// Storing any errors that happened to return
    pub failures: Vec<(u8, String)>,
}
