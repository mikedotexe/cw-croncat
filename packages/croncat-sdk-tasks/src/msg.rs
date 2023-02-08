use cosmwasm_schema::{cw_serde, QueryResponses};
use croncat_sdk_core::internal_messages::tasks::{TasksRemoveTaskByManager, TasksRescheduleTask};

use crate::types::TaskRequest;

#[cw_serde]
pub struct TasksInstantiateMsg {
    /// Chain name to add prefix to the task_hash
    pub chain_name: String,

    /// Assigned by Factory, denotes the version of this contract (CW2 spec) & used as the task verion as well.
    pub version: Option<String>,

    /// Address of the contract owner, defaults to the sender
    pub owner_addr: Option<String>,
    /// Name of the key for raw querying Manager address from the factory
    pub croncat_manager_key: (String, [u8; 2]),
    /// Name of the key for raw querying Agents address from the factory
    pub croncat_agents_key: (String, [u8; 2]),

    /// Time in nanos for each bucket of tasks
    pub slot_granularity_time: Option<u64>,

    /// Gas needed to cover proxy call without any action
    pub gas_base_fee: Option<u64>,
    /// Gas needed to cover single non-wasm task's Action
    pub gas_action_fee: Option<u64>,
    /// Gas needed to cover single query
    pub gas_query_fee: Option<u64>,

    /// Gas limit, to make sure task won't lock contract
    pub gas_limit: Option<u64>,
}

#[cw_serde]
pub struct UpdateConfigMsg {
    pub paused: Option<bool>,
    pub owner_addr: Option<String>,
    pub croncat_factory_addr: Option<String>,
    pub croncat_manager_key: Option<(String, [u8; 2])>,
    pub croncat_agents_key: Option<(String, [u8; 2])>,
    pub slot_granularity_time: Option<u64>,
    pub gas_base_fee: Option<u64>,
    pub gas_action_fee: Option<u64>,
    pub gas_query_fee: Option<u64>,
    pub gas_limit: Option<u64>,
}

#[cw_serde]
pub enum TasksExecuteMsg {
    UpdateConfig(UpdateConfigMsg),
    /// Allows any user or contract to pay for future txns based on a specific schedule
    /// contract, function id & other settings. When the task runs out of balance
    /// the task is no longer executed, any additional funds will be returned to task owner.
    CreateTask {
        task: Box<TaskRequest>,
    },

    /// Deletes a task in its entirety, returning any remaining balance to task owner.
    RemoveTask {
        task_hash: String,
    },
    // Methods for other internal contracts
    /// Remove task, used by the manager if task reached it's stop condition
    RemoveTaskByManager(TasksRemoveTaskByManager),
    /// Try to reschedule a task, if possible, used by the manager
    RescheduleTask(TasksRescheduleTask),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum TasksQueryMsg {
    #[returns(crate::types::Config)]
    Config {},
    /// Get the total amount of tasks
    #[returns(cosmwasm_std::Uint64)]
    TasksTotal {},
    /// returns the total task count & last task creation timestamp for agent nomination checks
    #[returns(crate::types::CurrentTaskInfoResponse)]
    CurrentTaskInfo {},
    /// Get next task to be done
    #[returns(crate::types::TaskResponse)]
    CurrentTask {},
    /// Get task by the task hash
    #[returns(crate::types::TaskResponse)]
    Task { task_hash: String },
    /// Get list of all tasks
    #[returns(Vec<crate::types::TaskInfo>)]
    Tasks {
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    #[returns(Vec<u64>)]
    EventedIds {
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    #[returns(Vec<String>)]
    EventedHashes {
        id: Option<u64>,
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    /// Get list of event driven tasks
    #[returns(Vec<crate::types::TaskInfo>)]
    EventedTasks {
        start: Option<u64>,
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    /// Get tasks created by the given address
    #[returns(Vec<crate::types::TaskResponse>)]
    TasksByOwner {
        owner_addr: String,
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    /// Simulate task_hash by the given task
    #[returns(String)]
    TaskHash { task: Box<crate::types::Task> },
    /// Get slot hashes by given slot
    #[returns(crate::types::SlotHashesResponse)]
    SlotHashes { slot: Option<u64> },
    // /// Get active slots
    // #[returns(crate::types::SlotIdsResponse)]
    // SlotIds {
    //     from_index: Option<u64>,
    //     limit: Option<u64>,
    // },
    #[returns(crate::types::SlotTasksTotalResponse)]
    SlotTasksTotal { offset: Option<u64> },
}
