/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.19.0.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

export type Addr = string;
export interface InstantiateMsg {
  agent_nomination_duration?: number | null;
  agents_eject_threshold?: number | null;
  allowed_agents?: string[] | null;
  croncat_manager_key: [string, [number, number]];
  croncat_tasks_key: [string, [number, number]];
  min_active_agent_count?: number | null;
  min_coins_for_agent_registration?: number | null;
  min_tasks_per_agent?: number | null;
  pause_admin: Addr;
  public_registration: boolean;
  version?: string | null;
}
export type ExecuteMsg = {
  add_agent_to_whitelist: {
    agent_address: string;
  };
} | {
  remove_agent_from_whitelist: {
    agent_address: string;
  };
} | {
  register_agent: {
    payable_account_id?: string | null;
  };
} | {
  update_agent: {
    payable_account_id: string;
  };
} | {
  check_in_agent: {};
} | {
  unregister_agent: {
    from_behind?: boolean | null;
  };
} | {
  on_task_created: AgentOnTaskCreated;
} | {
  on_task_completed: AgentOnTaskCompleted;
} | {
  update_config: {
    config: UpdateConfig;
  };
} | {
  tick: {};
} | {
  pause_contract: {};
} | {
  unpause_contract: {};
};
export interface AgentOnTaskCreated {}
export interface AgentOnTaskCompleted {
  agent_id: Addr;
  is_block_slot_task: boolean;
}
export interface UpdateConfig {
  agent_nomination_duration?: number | null;
  agents_eject_threshold?: number | null;
  croncat_manager_key?: [string, [number, number]] | null;
  croncat_tasks_key?: [string, [number, number]] | null;
  min_active_agent_count?: number | null;
  min_coins_for_agent_registration?: number | null;
  min_tasks_per_agent?: number | null;
  public_registration?: boolean | null;
}
export type QueryMsg = {
  get_agent: {
    account_id: string;
  };
} | {
  get_agent_ids: {
    from_index?: number | null;
    limit?: number | null;
  };
} | {
  get_approved_agent_addresses: {
    from_index?: number | null;
    limit?: number | null;
  };
} | {
  get_agent_tasks: {
    account_id: string;
  };
} | {
  config: {};
} | {
  paused: {};
};
export interface Config {
  agent_nomination_block_duration: number;
  agents_eject_threshold: number;
  croncat_factory_addr: Addr;
  croncat_manager_key: [string, [number, number]];
  croncat_tasks_key: [string, [number, number]];
  min_active_agent_count: number;
  min_coins_for_agent_registration: number;
  min_tasks_per_agent: number;
  owner_addr: Addr;
  pause_admin: Addr;
  public_registration: boolean;
}
export type Uint128 = string;
export type Timestamp = Uint64;
export type Uint64 = string;
export type AgentStatus = "active" | "pending" | "nominated";
export interface AgentResponse {
  agent?: AgentInfo | null;
}
export interface AgentInfo {
  balance: Uint128;
  last_executed_slot: number;
  payable_account_id: Addr;
  register_start: Timestamp;
  status: AgentStatus;
}
export interface GetAgentIdsResponse {
  active: Addr[];
  pending: Addr[];
}
export interface AgentTaskResponse {
  stats: TaskStats;
}
export interface TaskStats {
  num_block_tasks: Uint64;
  num_cron_tasks: Uint64;
}
export interface GetApprovedAgentAddresses {
  approved_addresses: Addr[];
}
export type Boolean = boolean;