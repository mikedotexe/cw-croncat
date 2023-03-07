use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp};
use std::fmt;

#[cw_serde]
pub struct AgentNominationStatus {
    pub start_height_of_nomination: Option<u64>,
    pub tasks_created_from_last_nomination: u64,
}

#[cw_serde]
pub enum AgentStatus {
    // Default for any new agent, if tasks ratio allows
    Active,

    // Default for any new agent, until more tasks come online
    Pending,

    // More tasks are available, agent must checkin to become active
    Nominated,
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AgentStatus::Active => write!(f, "active"),
            AgentStatus::Pending => write!(f, "pending"),
            AgentStatus::Nominated => write!(f, "nominated"),
        }
    }
}

#[cw_serde]
pub struct Agent {
    // Where rewards get transferred
    pub payable_account_id: Addr,

    // Timestamp of when agent first registered
    // Useful for rewarding agents for their patience while they are pending and operating service
    // Agent will be responsible to constantly monitor when it is their turn to join in active agent set (done as part of agent code loops)
    // Example data: 1633890060000000000 or 0
    pub register_start: Timestamp,
}

#[cw_serde]
#[derive(Default)]
pub struct AgentStats {
    pub completed_block_tasks: u64,
    pub completed_cron_tasks: u64,
    pub missed_blocked_tasks: u64,
    pub missed_cron_tasks: u64,
    // Holds slot number of the last slot when agent called proxy_call.
    // If agent does a task, this number is set to the current block.
    pub last_executed_slot: u64,
}
/// Contract configuration state
#[cw_serde]
pub struct Config {
    /// Address of the factory contract
    pub croncat_factory_addr: Addr,
    /// Name of the key for raw querying Manager address from the factory
    pub croncat_manager_key: (String, [u8; 2]),
    /// Name of the key for raw querying Tasks address from the factory
    pub croncat_tasks_key: (String, [u8; 2]),
    /// Contract owner address
    pub owner_addr: Addr,
    /// A multisig admin whose sole responsibility is to pause the contract in event of emergency.
    /// Must be a different contract address than DAO, cannot be a regular keypair
    /// Does not have the ability to unpause, must rely on the DAO to assess the situation and act accordingly
    pub pause_admin: Addr,
    /// Agent management
    /// The minimum number of tasks per agent
    /// Example: 10
    /// Explanation: For every 1 agent, 10 tasks per slot are available.
    /// NOTE: Caveat, when there are odd number of tasks or agents, the overflow will be available to first-come, first-serve. This doesn't negate the possibility of a failed txn from race case choosing winner inside a block.
    /// NOTE: The overflow will be adjusted to be handled by sweeper in next implementation.
    pub min_tasks_per_agent: u64,
    /// The duration a prospective agent has to nominate themselves.
    /// When a task is created such that a new agent can join,
    /// The agent at the zeroth index of the pending agent queue has this time to nominate
    /// The agent at the first index has twice this time to nominate (which would remove the former agent from the pending queue)
    /// Value is in seconds
    pub agent_nomination_block_duration: u16,
    /// Min coins that should be attached to register an agent
    pub min_coins_for_agent_registration: u64,
    /// How many slots an agent can miss before being removed from the active queue
    pub agents_eject_threshold: u64,
    /// Minimum agent count in active queue to be untouched by bad agent verifier
    pub min_active_agent_count: u16,
    /// Whether agent registration is public or restricted to an internal whitelist (allowed_agents)
    /// Determines whether agent registration is open to the public
    /// If false, the APPROVED_AGENTS map will determine if an agent is allowed to register
    /// If true, any address can register and enter the pending queue,
    /// provided they have the assets required.
    /// Note that once this becomes true, it's intentionally meant to be true forever,
    /// since this is an aspect of progressive decentralization
    pub public_registration: bool,
}

#[cfg(test)]
mod test {
    use crate::types::AgentStatus;

    #[test]
    fn agent_status_fmt() {
        let active = AgentStatus::Active;
        assert_eq!(format!("{active}"), "active");

        let nominated = AgentStatus::Nominated;
        assert_eq!(format!("{nominated}"), "nominated");

        let pending = AgentStatus::Pending;
        assert_eq!(format!("{pending}"), "pending");
    }
}
