use crate::types::AgentStatus;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Timestamp, Uint128, Uint64};
use croncat_sdk_core::internal_messages::agents::{AgentOnTaskCompleted, AgentOnTaskCreated};

#[cw_serde]
pub struct InstantiateMsg {
    /// A multisig admin whose sole responsibility is to pause the contract in event of emergency.
    /// Must be a different contract address than DAO, cannot be a regular keypair
    /// Does not have the ability to unpause, must rely on the DAO to assess the situation and act accordingly
    pub pause_admin: Addr,

    /// CW2 Version provided by factory
    pub version: Option<String>,

    /// Name of the key for raw querying Manager address from the factory
    pub croncat_manager_key: (String, [u8; 2]),

    /// Name of the key for raw querying Tasks address from the factory
    pub croncat_tasks_key: (String, [u8; 2]),

    /// Sets the amount of time opportunity for a pending agent to become active.
    /// If there is a pending queue, the longer a pending agent waits,
    /// the more pending agents can potentially become active based on this nomination window.
    /// This duration doesn't block the already nominated agent from becoming active,
    /// it only opens the door for more to become active. If a pending agent is nominated,
    /// then is lazy and beat by another agent, they get removed from pending queue and must
    /// register again.
    pub agent_nomination_duration: Option<u16>,

    /// The ratio used to calculate active agents/tasks. Example: "3", requires there are
    /// 4 tasks before letting in another agent to become active. (3 tasks for agent 1, 1 task for agent 2)
    pub min_tasks_per_agent: Option<u64>,

    /// The required amount needed to actually execute a few tasks before withdraw profits.
    /// This helps make sure agent wont get stuck out the gate
    pub min_coins_for_agent_registration: Option<u64>,

    /// How many slots an agent can miss before being removed from the active queue
    pub agents_eject_threshold: Option<u64>,

    /// Minimum agent count in active queue to be untouched by bad agent verifier
    pub min_active_agent_count: Option<u16>,

    /// Whether agent registration is public or restricted to an internal whitelist
    pub public_registration: bool,

    /// If public registration is false, this provides initial, approved agent addresses
    pub allowed_agents: Option<Vec<String>>,
}

/// Execute messages for agent contract
#[cw_serde]
pub enum ExecuteMsg {
    /// Adds an agent address to the internal whitelist
    AddAgentToWhitelist { agent_address: String },
    /// Removes an agent from the whitelist
    /// Note: this does not kick the agent, but instead means they will not be able to re-register
    RemoveAgentFromWhitelist { agent_address: String },
    /// Action registers new agent
    RegisterAgent { payable_account_id: Option<String> },
    /// Action for updating agents
    UpdateAgent { payable_account_id: String },
    /// Action moves agent from pending to active list
    CheckInAgent {},
    /// Actions for removing agent from the system
    UnregisterAgent { from_behind: Option<bool> },
    /// Task contract will send message when task is created
    OnTaskCreated(AgentOnTaskCreated),
    /// Task contract will send message when task is completed
    OnTaskCompleted(AgentOnTaskCompleted),
    /// Action for updating agent contract configuration
    UpdateConfig { config: UpdateConfig },
    /// Tick action will remove unactive agents periodically or do and any other internal cron tasks
    Tick {},
    /// Pauses all operations for this contract, can only be done by pause_admin
    PauseContract {},
    /// unpauses all operations for this contract, can only be unpaused by owner_addr
    UnpauseContract {},
}

/// Agent request response
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get an agent by specified account_id, returns AgentInfo if found
    #[returns[AgentResponse]]
    GetAgent { account_id: String },
    /// Gets the id list of agents, pagination is supported
    #[returns[GetAgentIdsResponse]]
    GetAgentIds {
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    /// Gets the approved agents' addresses, pagination is supported
    /// This only applies when Config's `public_registration` is false
    #[returns[ApprovedAgentAddresses]]
    GetApprovedAgentAddresses {
        from_index: Option<u64>,
        limit: Option<u64>,
    },
    /// Gets the specified agent tasks
    #[returns[AgentTaskResponse]]
    GetAgentTasks { account_id: String },
    /// Gets the agent contract configuration
    #[returns[crate::types::Config]]
    Config {},

    /// Helper for query responses on versioned contracts
    #[returns[bool]]
    Paused {},
}
/// Response containing active/pending agents
#[cw_serde]
pub struct GetAgentIdsResponse {
    /// Active agent list
    pub active: Vec<Addr>,
    /// Pending agent list
    pub pending: Vec<Addr>,
}
/// Response containing approved agents' addresses
#[cw_serde]
pub struct ApprovedAgentAddresses {
    /// Active agent list
    pub approved_addresses: Vec<Addr>,
}
/// Agent data
#[cw_serde]
pub struct AgentInfo {
    /// Agent status
    pub status: AgentStatus,
    /// Account where agent will move all his rewards
    pub payable_account_id: Addr,
    /// Agent balance
    pub balance: Uint128,
    /// Last executed slot number
    pub last_executed_slot: u64,
    /// Registration time
    pub register_start: Timestamp,
}
/// Agent response containing agent information
#[cw_serde]
pub struct AgentResponse {
    /// Agent data
    pub agent: Option<AgentInfo>,
}
/// Agent statistics data
#[cw_serde]
pub struct TaskStats {
    /// Total block tasks for specified agent
    pub num_block_tasks: Uint64,
    /// Total cron tasks for specified agent
    pub num_cron_tasks: Uint64,
}
/// Agent task response for getting stats and task information
#[cw_serde]
pub struct AgentTaskResponse {
    /// Agent tasks statistic information
    pub stats: TaskStats,
}
/// Updatable agents contract configuration
#[cw_serde]
pub struct UpdateConfig {
    /// Name of the key for raw querying Manager address from the factory
    pub croncat_manager_key: Option<(String, [u8; 2])>,

    /// Name of the key for raw querying Tasks address from the factory
    pub croncat_tasks_key: Option<(String, [u8; 2])>,

    /// Minimum tasks count to be reached by agent before next agent nomination
    pub min_tasks_per_agent: Option<u64>,

    /// Duration to be passed before next agent nomination
    pub agent_nomination_duration: Option<u16>,

    /// Minimum funds to be attached for agent registration
    pub min_coins_for_agent_registration: Option<u64>,

    /// How many slots an agent can miss before being removed from the active queue
    pub agents_eject_threshold: Option<u64>,

    /// Minimum agent count in active queue to be untouched by bad agent verifier
    pub min_active_agent_count: Option<u16>,

    /// Determines whether agent registration is public or uses the whitelist (APPROVED_AGENTS Map)
    pub public_registration: Option<bool>,
}
