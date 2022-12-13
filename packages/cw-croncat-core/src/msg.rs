use crate::types::{
    Action, AgentStatus, Boundary, BoundaryValidated, GasFraction, GenericBalance, Interval, Task,
    Transform,
};
use crate::types::{Agent, SlotType};
use cosmwasm_std::{Addr, Coin, Timestamp, Uint64};
use cw20::{Balance, Cw20Coin, Cw20CoinVerified};
use cw_rules_core::types::CroncatQuery;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// NOTE: Which version is more practical?
// // Exporting a nice schema
// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// pub enum Croncat {
//     Agent(Agent),
//     Task(Task),
//     ConfigResponse(GetConfigResponse),
//     BalancesResponse(GetBalancesResponse),
//     GetAgentIdsResponse(GetAgentIdsResponse),
//     GetAgentTasksResponse(GetAgentTasksResponse),
//     TaskRequest(TaskRequest),
//     TaskResponse(TaskResponse),
//     ValidateIntervalResponse(ValidateIntervalResponse),
//     GetAgentResponse(GetAgentResponse),
//     GetTasksResponse(GetTasksResponse),
//     GetTasksByOwnerResponse(GetTasksByOwnerResponse),
//     GetTaskResponse(GetTaskResponse),
//     GetTaskHashResponse(GetTaskHashResponse),
//     GetSlotHashesResponse(GetSlotHashesResponse),
//     GetSlotIdsResponse(GetSlotIdsResponse),
// }

// Exporting a nice schema
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub struct Croncat {
    pub(crate) agent: Option<Agent>,
    pub(crate) task: Option<Task>,
    pub(crate) config_response: Option<GetConfigResponse>,
    pub(crate) balance_response: Option<GetBalancesResponse>,
    pub(crate) get_agent_ids_response: Option<GetAgentIdsResponse>,
    pub(crate) get_agent_tasks_response: Option<AgentTaskResponse>,
    pub(crate) task_request: Option<TaskRequest>,
    pub(crate) task_response: Option<TaskResponse>,
    pub(crate) validate_interval_response: Option<bool>,
    pub(crate) get_agent_response: Option<Option<AgentResponse>>,
    pub(crate) get_tasks_response: Option<Vec<TaskResponse>>,
    pub(crate) get_tasks_by_owner_response: Option<Vec<TaskResponse>>,
    pub(crate) get_task_response: Option<Option<TaskResponse>>,
    pub(crate) get_task_hash_response: Option<String>,
    pub(crate) get_slot_hashes_response: Option<GetSlotHashesResponse>,
    pub(crate) get_slot_ids_response: Option<GetSlotIdsResponse>,
    pub(crate) get_wallet_balances_response: Option<GetWalletBalancesResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    // TODO: Submit issue for AppBuilder tests not working for -- deps.querier.query_bonded_denom()?;
    pub denom: String,
    pub cw_rules_addr: String,
    pub owner_id: Option<String>,
    pub gas_base_fee: Option<Uint64>,
    pub gas_action_fee: Option<Uint64>,
    pub gas_fraction: Option<GasFraction>,
    pub agent_nomination_duration: Option<u16>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateSettings {
        owner_id: Option<String>,
        slot_granularity_time: Option<u64>,
        paused: Option<bool>,
        agent_fee: Option<u64>,
        gas_base_fee: Option<Uint64>,
        gas_action_fee: Option<Uint64>,
        gas_fraction: Option<GasFraction>,
        proxy_callback_gas: Option<u32>,
        min_tasks_per_agent: Option<u64>,
        agents_eject_threshold: Option<u64>,
        // treasury_id: Option<String>,
    },
    MoveBalances {
        balances: Vec<Balance>,
        account_id: String,
    },

    RegisterAgent {
        payable_account_id: Option<String>,
    },
    UpdateAgent {
        payable_account_id: String,
    },
    CheckInAgent {},
    UnregisterAgent {
        from_behind: Option<bool>,
    },
    WithdrawReward {},

    CreateTask {
        task: TaskRequest,
    },
    RemoveTask {
        task_hash: String,
    },
    RefillTaskBalance {
        task_hash: String,
    },
    RefillTaskCw20Balance {
        task_hash: String,
        cw20_coins: Vec<Cw20Coin>,
    },
    ProxyCall {
        task_hash: Option<String>,
    },
    /// Receive cw20 token
    Receive(cw20::Cw20ReceiveMsg),
    WithdrawWalletBalance {
        cw20_amounts: Vec<Cw20Coin>,
    },
    Tick {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetConfig {},
    GetBalances {},
    GetAgent {
        account_id: String,
    },
    GetAgentIds {},
    GetAgentTasks {
        account_id: String,
    },
    GetTasks {
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    GetTasksWithQueries {
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    GetTasksByOwner {
        owner_id: String,
    },
    GetTask {
        task_hash: String,
    },
    GetTaskHash {
        task: Box<Task>,
    },
    ValidateInterval {
        interval: Interval,
    },
    GetSlotHashes {
        slot: Option<u64>,
    },
    GetSlotIds {},
    GetWalletBalances {
        wallet: String,
    },
    GetState {
        from_index: Option<u64>,
        limit: Option<u64>,
    },

    SimulateTask {
        task: TaskRequest,
        funds: GenericBalance,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetConfigResponse {
    pub paused: bool,
    pub owner_id: Addr,
    // pub treasury_id: Option<Addr>,
    pub min_tasks_per_agent: u64,
    pub agents_eject_threshold: u64,
    pub agent_active_indices: Vec<(SlotType, u32, u32)>,
    pub agent_nomination_duration: u16,

    pub cw_rules_addr: Addr,

    pub agent_fee: u64,
    pub gas_fraction: GasFraction,
    pub gas_base_fee: u64,
    pub gas_action_fee: u64,
    pub proxy_callback_gas: u32,
    pub slot_granularity_time: u64,

    pub cw20_whitelist: Vec<Addr>,
    pub native_denom: String,
    pub available_balance: GenericBalance, // tasks + rewards balances
    pub staked_balance: GenericBalance, // surplus that is temporary staking (to be used in conjunction with external treasury)

    // The default amount of tasks to query
    pub limit: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetBalancesResponse {
    pub native_denom: String,
    pub available_balance: GenericBalance,
    pub staked_balance: GenericBalance,
    pub cw20_whitelist: Vec<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetWalletBalancesResponse {
    pub cw20_balances: Vec<Cw20CoinVerified>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct GetAgentIdsResponse {
    pub active: Vec<Addr>,
    pub pending: Vec<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AgentResponse {
    // This field doesn't exist in the Agent struct and is the only one that differs
    pub status: AgentStatus,
    pub payable_account_id: Addr,
    pub balance: GenericBalance,
    pub total_tasks_executed: u64,
    pub last_executed_slot: u64,
    pub register_start: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AgentTaskResponse {
    pub num_block_tasks: Uint64,
    pub num_block_tasks_extra: Uint64,
    pub num_cron_tasks: Uint64,
    pub num_cron_tasks_extra: Uint64,
}

impl AgentTaskResponse {
    pub fn has_any_slot_tasks(&self, slot_kind: SlotType) -> bool {
        if self.num_of_slot_tasks(slot_kind) < 1u64 {
            return false;
        }
        true
    }
    pub fn num_of_slot_tasks(&self, slot_kind: SlotType) -> u64 {
        if slot_kind == SlotType::Block {
            return self.num_block_tasks.u64();
        }

        self.num_cron_tasks.u64()
    }
    pub fn has_any_slot_extra_tasks(&self, slot_kind: SlotType) -> bool {
        if self.num_of_slot_extra_tasks(slot_kind) < 1u64 {
            return false;
        }
        true
    }
    pub fn num_of_slot_extra_tasks(&self, slot_kind: SlotType) -> u64 {
        if slot_kind == SlotType::Block {
            return self.num_block_tasks_extra.u64();
        }

        self.num_cron_tasks_extra.u64()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TaskRequest {
    pub interval: Interval,
    pub boundary: Option<Boundary>,
    pub stop_on_fail: bool,
    pub actions: Vec<Action>,
    pub queries: Option<Vec<CroncatQuery>>,
    pub transforms: Option<Vec<Transform>>,
    pub cw20_coins: Vec<Cw20Coin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TaskResponse {
    pub task_hash: String,

    pub owner_id: Addr,

    pub interval: Interval,
    pub boundary: Option<Boundary>,

    pub stop_on_fail: bool,
    pub total_deposit: Vec<Coin>,
    pub total_cw20_deposit: Vec<Cw20CoinVerified>,
    pub amount_for_one_task_native: Vec<Coin>,
    pub amount_for_one_task_cw20: Vec<Cw20CoinVerified>,

    pub actions: Vec<Action>,
    pub queries: Option<Vec<CroncatQuery>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TaskWithQueriesResponse {
    pub task_hash: String,
    pub interval: Interval,
    pub boundary: Option<Boundary>,
    pub queries: Option<Vec<CroncatQuery>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CwCroncatResponse {
    pub config: GetConfigResponse,

    pub agent_active_queue: Vec<Addr>,
    pub agent_pending_queue: Vec<Addr>,
    pub agents: Vec<AgentResponse>,

    pub tasks: Vec<TaskResponse>,
    pub task_total: Uint64,

    pub time_slots: Vec<SlotResponse>,
    pub block_slots: Vec<SlotResponse>,
    pub tasks_with_queries: Vec<TaskWithQueriesResponse>,
    pub tasks_with_queries_total: Uint64,

    pub time_slots_queries: Vec<SlotWithQueriesResponse>,
    pub block_slots_queries: Vec<SlotWithQueriesResponse>,

    pub reply_index: Uint64,

    pub agent_nomination_begin_time: Option<Timestamp>,

    pub balancer_mode: RoundRobinBalancerModeResponse,
    pub balances: Vec<BalancesResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct SlotResponse {
    pub slot: Uint64,
    pub tasks: Vec<Vec<u8>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BalancesResponse {
    pub address: Addr,
    pub balances: Vec<Cw20CoinVerified>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum RoundRobinBalancerModeResponse {
    ActivationOrder,
    Equalizer,
}

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
// pub struct ReplyQueueResponse {
//     pub index: Uint64,
//     pub item: QueueItemResponse,
// }

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
// pub struct QueueItemResponse {
//     pub contract_addr: Option<Addr>,
//     pub action_idx: Uint64,
//     pub task_hash: Option<Vec<u8>>,
//     pub task_is_extra: Option<bool>,
//     pub agent_id: Option<Addr>,
//     pub failed: bool,
// }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct SlotWithQueriesResponse {
    pub task_hash: Vec<u8>,
    pub slot: Uint64,
}

impl From<Task> for TaskResponse {
    fn from(task: Task) -> Self {
        let boundary = match (task.boundary, &task.interval) {
            (
                BoundaryValidated {
                    start: None,
                    end: None,
                },
                _,
            ) => None,
            (BoundaryValidated { start, end }, Interval::Cron(_)) => Some(Boundary::Time {
                start: start.map(Timestamp::from_nanos),
                end: end.map(Timestamp::from_nanos),
            }),
            (BoundaryValidated { start, end }, _) => Some(Boundary::Height {
                start: start.map(Into::into),
                end: end.map(Into::into),
            }),
        };
        TaskResponse {
            task_hash: task.to_hash(),
            owner_id: task.owner_id,
            interval: task.interval,
            boundary,
            stop_on_fail: task.stop_on_fail,
            total_deposit: task.total_deposit.native,
            total_cw20_deposit: task.total_deposit.cw20,
            amount_for_one_task_native: task.amount_for_one_task.native,
            amount_for_one_task_cw20: task.amount_for_one_task.cw20,
            actions: task.actions,
            queries: task.queries,
        }
    }
}

impl From<Task> for TaskWithQueriesResponse {
    fn from(task: Task) -> Self {
        let boundary = match (task.boundary, &task.interval) {
            (
                BoundaryValidated {
                    start: None,
                    end: None,
                },
                _,
            ) => None,
            (BoundaryValidated { start, end }, Interval::Cron(_)) => Some(Boundary::Time {
                start: start.map(Timestamp::from_nanos),
                end: end.map(Timestamp::from_nanos),
            }),
            (BoundaryValidated { start, end }, _) => Some(Boundary::Height {
                start: start.map(Into::into),
                end: end.map(Into::into),
            }),
        };
        TaskWithQueriesResponse {
            task_hash: task.to_hash(),
            interval: task.interval,
            boundary,
            queries: task.queries,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct GetSlotHashesResponse {
    pub block_id: u64,
    pub block_task_hash: Vec<String>,
    pub time_id: u64,
    pub time_task_hash: Vec<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct SimulateTaskResponse {
    pub estimated_gas: u64,
    pub occurrences: u64,
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct GetSlotIdsResponse {
    pub time_ids: Vec<u64>,
    pub block_ids: Vec<u64>,
}

// cw_rules
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct QueryConstruct {
    pub queries: Vec<CroncatQuery>,
}
