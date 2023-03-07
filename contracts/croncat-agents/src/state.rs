use crate::distributor::AgentTaskDistributor;
use crate::msg::*;
use cosmwasm_std::{Addr, Empty};
use croncat_sdk_agents::types::{AgentNominationStatus, Config};
use cw_storage_plus::{Deque, Item, Map};

/// Contract config, just the owner address for now, preferably dao
pub const CONFIG: Item<Config> = Item::new("agents_config");

/// Controls whether or not the contract is paused. Can only be changed to TRUE by
/// the pause_admin, can only be unpaused by DAO/owner_addr
pub const PAUSED: Item<bool> = Item::new("paused");

pub(crate) const DEFAULT_NOMINATION_BLOCK_DURATION: u16 = 10;
pub(crate) const DEFAULT_MIN_TASKS_PER_AGENT: u64 = 3;
pub(crate) const DEFAULT_MIN_COINS_FOR_AGENT_REGISTRATION: u64 = 200_000;

pub const AGENTS: Map<&Addr, Agent> = Map::new("agents");
pub const AGENTS_ACTIVE: Item<Vec<Addr>> = Item::new("agents_active");
pub const AGENTS_PENDING: Deque<Addr> = Deque::new("agents_pending");
pub const AGENT_STATS: Map<&Addr, AgentStats> = Map::new("agent_stats");
/// Due to the absence of a Set data structure, we use a Map that points to Empty
/// This will only be used if the Config's `public_registration` value is false
pub const APPROVED_AGENTS: Map<&Addr, Empty> = Map::new("approved_agents");
pub const AGENT_NOMINATION_STATUS: Item<AgentNominationStatus> =
    Item::new("agent_nomination_status");

pub const AGENT_TASK_DISTRIBUTOR: AgentTaskDistributor = AgentTaskDistributor::new();
pub const DEFAULT_AGENTS_EJECT_THRESHOLD: u64 = 600;
pub const DEFAULT_MIN_ACTIVE_AGENT_COUNT: u16 = 1;
pub const DEFAULT_PUBLIC_REGISTRATION_ENABLED: bool = false;
