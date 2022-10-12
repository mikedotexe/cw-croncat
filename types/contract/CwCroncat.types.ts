/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.14.2.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

export type Addr = string;
export type Uint128 = string;
export type Timestamp = Uint64;
export type Uint64 = string;
export type SlotType = "Block" | "Cron";
export type AgentStatus = "Active" | "Pending" | "Nominated";
export type CosmosMsgForEmpty = {
  bank: BankMsg;
} | {
  custom: Empty;
} | {
  staking: StakingMsg;
} | {
  distribution: DistributionMsg;
} | {
  stargate: {
    type_url: string;
    value: Binary;
    [k: string]: unknown;
  };
} | {
  ibc: IbcMsg;
} | {
  wasm: WasmMsg;
} | {
  gov: GovMsg;
};
export type BankMsg = {
  send: {
    amount: Coin[];
    to_address: string;
    [k: string]: unknown;
  };
} | {
  burn: {
    amount: Coin[];
    [k: string]: unknown;
  };
};
export type StakingMsg = {
  delegate: {
    amount: Coin;
    validator: string;
    [k: string]: unknown;
  };
} | {
  undelegate: {
    amount: Coin;
    validator: string;
    [k: string]: unknown;
  };
} | {
  redelegate: {
    amount: Coin;
    dst_validator: string;
    src_validator: string;
    [k: string]: unknown;
  };
};
export type DistributionMsg = {
  set_withdraw_address: {
    address: string;
    [k: string]: unknown;
  };
} | {
  withdraw_delegator_reward: {
    validator: string;
    [k: string]: unknown;
  };
};
export type Binary = string;
export type IbcMsg = {
  transfer: {
    amount: Coin;
    channel_id: string;
    timeout: IbcTimeout;
    to_address: string;
    [k: string]: unknown;
  };
} | {
  send_packet: {
    channel_id: string;
    data: Binary;
    timeout: IbcTimeout;
    [k: string]: unknown;
  };
} | {
  close_channel: {
    channel_id: string;
    [k: string]: unknown;
  };
};
export type WasmMsg = {
  execute: {
    contract_addr: string;
    funds: Coin[];
    msg: Binary;
    [k: string]: unknown;
  };
} | {
  instantiate: {
    admin?: string | null;
    code_id: number;
    funds: Coin[];
    label: string;
    msg: Binary;
    [k: string]: unknown;
  };
} | {
  migrate: {
    contract_addr: string;
    msg: Binary;
    new_code_id: number;
    [k: string]: unknown;
  };
} | {
  update_admin: {
    admin: string;
    contract_addr: string;
    [k: string]: unknown;
  };
} | {
  clear_admin: {
    contract_addr: string;
    [k: string]: unknown;
  };
};
export type GovMsg = {
  vote: {
    proposal_id: number;
    vote: VoteOption;
    [k: string]: unknown;
  };
};
export type VoteOption = "yes" | "no" | "abstain" | "no_with_veto";
export type Boundary = {
  Height: {
    end?: Uint64 | null;
    start?: Uint64 | null;
    [k: string]: unknown;
  };
} | {
  Time: {
    end?: Timestamp | null;
    start?: Timestamp | null;
    [k: string]: unknown;
  };
};
export type Interval = ("Once" | "Immediate") | {
  Block: number;
} | {
  Cron: string;
};
export type Rule = {
  has_balance_gte: HasBalanceGte;
} | {
  check_owner_of_nft: CheckOwnerOfNft;
} | {
  check_proposal_status: CheckProposalStatus;
} | {
  generic_query: GenericQuery;
};
export type Balance = {
  native: NativeBalance;
} | {
  cw20: Cw20CoinVerified;
};
export type NativeBalance = Coin[];
export type Status = "open" | "rejected" | "passed" | "executed" | "closed" | "execution_failed";
export type ValueIndex = {
  key: string;
} | {
  index: number;
};
export type ValueOrdering = "unit_above" | "unit_above_equal" | "unit_below" | "unit_below_equal" | "equal";
export interface Croncat {
  Agent?: Agent | null;
  BalanceResponse?: GetBalancesResponse | null;
  ConfigResponse?: GetConfigResponse | null;
  GetAgentIdsResponse?: GetAgentIdsResponse | null;
  GetAgentResponse?: (AgentResponse | null) | null;
  GetAgentTasksResponse?: AgentTaskResponse | null;
  GetSlotHashesResponse?: GetSlotHashesResponse | null;
  GetSlotIdsResponse?: GetSlotIdsResponse | null;
  GetTaskHashResponse?: string | null;
  GetTaskResponse?: (TaskResponse | null) | null;
  GetTasksByOwnerResponse?: TaskResponse[] | null;
  GetTasksResponse?: TaskResponse[] | null;
  GetWalletBalancesResponse?: GetWalletBalancesResponse | null;
  Task?: Task | null;
  TaskRequest?: TaskRequest | null;
  TaskResponse?: TaskResponse | null;
  ValidateIntervalResponse?: boolean | null;
  [k: string]: unknown;
}
export interface Agent {
  balance: GenericBalance;
  last_missed_slot: number;
  payable_account_id: Addr;
  register_start: Timestamp;
  total_tasks_executed: number;
  [k: string]: unknown;
}
export interface GenericBalance {
  cw20: Cw20CoinVerified[];
  native: Coin[];
  [k: string]: unknown;
}
export interface Cw20CoinVerified {
  address: Addr;
  amount: Uint128;
  [k: string]: unknown;
}
export interface Coin {
  amount: Uint128;
  denom: string;
  [k: string]: unknown;
}
export interface GetBalancesResponse {
  available_balance: GenericBalance;
  cw20_whitelist: Addr[];
  native_denom: string;
  staked_balance: GenericBalance;
  [k: string]: unknown;
}
export interface GetConfigResponse {
  agent_active_indices: [SlotType, number, number][];
  agent_fee: number;
  agents_eject_threshold: number;
  cw_rules_addr: Addr;
  gas_fraction: GasFraction;
  min_tasks_per_agent: number;
  native_denom: string;
  owner_id: Addr;
  paused: boolean;
  proxy_callback_gas: number;
  slot_granularity: number;
  [k: string]: unknown;
}
export interface GasFraction {
  denominator: number;
  numerator: number;
  [k: string]: unknown;
}
export interface GetAgentIdsResponse {
  active: Addr[];
  pending: Addr[];
  [k: string]: unknown;
}
export interface AgentResponse {
  balance: GenericBalance;
  last_missed_slot: number;
  payable_account_id: Addr;
  register_start: Timestamp;
  status: AgentStatus;
  total_tasks_executed: number;
  [k: string]: unknown;
}
export interface AgentTaskResponse {
  num_block_tasks: Uint64;
  num_block_tasks_extra: Uint64;
  num_cron_tasks: Uint64;
  num_cron_tasks_extra: Uint64;
  [k: string]: unknown;
}
export interface GetSlotHashesResponse {
  block_id: number;
  block_task_hash: string[];
  time_id: number;
  time_task_hash: string[];
  [k: string]: unknown;
}
export interface GetSlotIdsResponse {
  block_ids: number[];
  time_ids: number[];
  [k: string]: unknown;
}
export interface TaskResponse {
  actions: ActionForEmpty[];
  boundary?: Boundary | null;
  interval: Interval;
  owner_id: Addr;
  rules?: Rule[] | null;
  stop_on_fail: boolean;
  task_hash: string;
  total_cw20_deposit: Cw20CoinVerified[];
  total_deposit: Coin[];
  [k: string]: unknown;
}
export interface ActionForEmpty {
  gas_limit?: number | null;
  msg: CosmosMsgForEmpty;
  [k: string]: unknown;
}
export interface Empty {
  [k: string]: unknown;
}
export interface IbcTimeout {
  block?: IbcTimeoutBlock | null;
  timestamp?: Timestamp | null;
  [k: string]: unknown;
}
export interface IbcTimeoutBlock {
  height: number;
  revision: number;
  [k: string]: unknown;
}
export interface HasBalanceGte {
  address: string;
  required_balance: Balance;
  [k: string]: unknown;
}
export interface CheckOwnerOfNft {
  address: string;
  nft_address: string;
  token_id: string;
  [k: string]: unknown;
}
export interface CheckProposalStatus {
  dao_address: string;
  proposal_id: number;
  status: Status;
  [k: string]: unknown;
}
export interface GenericQuery {
  contract_addr: string;
  gets: ValueIndex[];
  msg: Binary;
  ordering: ValueOrdering;
  value: Binary;
  [k: string]: unknown;
}
export interface GetWalletBalancesResponse {
  cw20_balances: Cw20CoinVerified[];
  [k: string]: unknown;
}
export interface Task {
  actions: ActionForEmpty[];
  amount_for_one_task: GenericBalance;
  boundary: BoundaryValidated;
  funds_withdrawn_recurring: Uint128;
  interval: Interval;
  owner_id: Addr;
  rules?: Rule[] | null;
  stop_on_fail: boolean;
  total_deposit: GenericBalance;
  [k: string]: unknown;
}
export interface BoundaryValidated {
  end?: number | null;
  start?: number | null;
  [k: string]: unknown;
}
export interface TaskRequest {
  actions: ActionForEmpty[];
  boundary?: Boundary | null;
  cw20_coins: Cw20Coin[];
  interval: Interval;
  rules?: Rule[] | null;
  stop_on_fail: boolean;
  [k: string]: unknown;
}
export interface Cw20Coin {
  address: string;
  amount: Uint128;
  [k: string]: unknown;
}
export type ExecuteMsg = {
  update_settings: {
    agent_fee?: number | null;
    agents_eject_threshold?: number | null;
    gas_action_fee?: Uint64 | null;
    gas_base_fee?: Uint64 | null;
    gas_fraction?: GasFraction | null;
    min_tasks_per_agent?: number | null;
    owner_id?: string | null;
    paused?: boolean | null;
    proxy_callback_gas?: number | null;
    slot_granularity?: number | null;
    [k: string]: unknown;
  };
} | {
  move_balances: {
    account_id: string;
    balances: Balance[];
    [k: string]: unknown;
  };
} | {
  register_agent: {
    payable_account_id?: string | null;
    [k: string]: unknown;
  };
} | {
  update_agent: {
    payable_account_id: string;
    [k: string]: unknown;
  };
} | {
  check_in_agent: {
    [k: string]: unknown;
  };
} | {
  unregister_agent: {
    [k: string]: unknown;
  };
} | {
  withdraw_reward: {
    [k: string]: unknown;
  };
} | {
  create_task: {
    task: TaskRequest;
    [k: string]: unknown;
  };
} | {
  remove_task: {
    task_hash: string;
    [k: string]: unknown;
  };
} | {
  refill_task_balance: {
    task_hash: string;
    [k: string]: unknown;
  };
} | {
  refill_task_cw20_balance: {
    cw20_coins: Cw20Coin[];
    task_hash: string;
    [k: string]: unknown;
  };
} | {
  proxy_call: {
    task_hash?: string | null;
    [k: string]: unknown;
  };
} | {
  receive: Cw20ReceiveMsg;
} | {
  withdraw_wallet_balance: {
    cw20_amounts: Cw20Coin[];
    [k: string]: unknown;
  };
};
export interface Cw20ReceiveMsg {
  amount: Uint128;
  msg: Binary;
  sender: string;
  [k: string]: unknown;
}
export type GetAgentResponse = AgentResponse | null;
export type GetAgentTasksResponse = TaskResponse | null;
export type RoundRobinBalancerModeResponse = "ActivationOrder" | "Equalizer";
export interface GetStateResponse {
  agent_active_queue: Addr[];
  agent_nomination_begin_time?: Timestamp | null;
  agent_pending_queue: Addr[];
  balancer_mode: RoundRobinBalancerModeResponse;
  balances: BalancesResponse[];
  block_slots: SlotResponse[];
  block_slots_rules: SlotWithRuleResponse[];
  config: GetConfigResponse;
  reply_index: Uint64;
  reply_queue: ReplyQueueResponse[];
  task_total: Uint64;
  tasks: TaskResponse[];
  tasks_with_rules: TaskWithRulesResponse[];
  tasks_with_rules_total: Uint64;
  time_slots: SlotResponse[];
  time_slots_rules: SlotWithRuleResponse[];
  [k: string]: unknown;
}
export interface BalancesResponse {
  address: Addr;
  balances: Cw20CoinVerified[];
  [k: string]: unknown;
}
export interface SlotResponse {
  slot: Uint64;
  tasks: number[][];
  [k: string]: unknown;
}
export interface SlotWithRuleResponse {
  slot: Uint64;
  task_hash: number[];
  [k: string]: unknown;
}
export interface ReplyQueueResponse {
  index: Uint64;
  item: QueueItemResponse;
  [k: string]: unknown;
}
export interface QueueItemResponse {
  action_idx: Uint64;
  agent_id?: Addr | null;
  contract_addr?: Addr | null;
  failed: boolean;
  task_hash?: number[] | null;
  task_is_extra?: boolean | null;
  [k: string]: unknown;
}
export interface TaskWithRulesResponse {
  boundary?: Boundary | null;
  interval: Interval;
  rules?: Rule[] | null;
  task_hash: string;
  [k: string]: unknown;
}
export type GetTaskHashResponse = string;
export type GetTaskResponse = TaskResponse | null;
export type GetTasksByOwnerResponse = TaskResponse[];
export type GetTasksResponse = TaskResponse[];
export type GetTasksWithRulesResponse = TaskWithRulesResponse[];
export interface InstantiateMsg {
  agent_nomination_duration?: number | null;
  cw_rules_addr: string;
  denom: string;
  gas_action_fee?: Uint64 | null;
  gas_base_fee?: Uint64 | null;
  gas_fraction?: GasFraction | null;
  owner_id?: string | null;
  [k: string]: unknown;
}
export type QueryMsg = {
  get_config: {
    [k: string]: unknown;
  };
} | {
  get_balances: {
    [k: string]: unknown;
  };
} | {
  get_agent: {
    account_id: string;
    [k: string]: unknown;
  };
} | {
  get_agent_ids: {
    [k: string]: unknown;
  };
} | {
  get_agent_tasks: {
    account_id: string;
    [k: string]: unknown;
  };
} | {
  get_tasks: {
    from_index?: number | null;
    limit?: number | null;
    [k: string]: unknown;
  };
} | {
  get_tasks_with_rules: {
    from_index?: number | null;
    limit?: number | null;
    [k: string]: unknown;
  };
} | {
  get_tasks_by_owner: {
    owner_id: string;
    [k: string]: unknown;
  };
} | {
  get_task: {
    task_hash: string;
    [k: string]: unknown;
  };
} | {
  get_task_hash: {
    task: Task;
    [k: string]: unknown;
  };
} | {
  validate_interval: {
    interval: Interval;
    [k: string]: unknown;
  };
} | {
  get_slot_hashes: {
    slot?: number | null;
    [k: string]: unknown;
  };
} | {
  get_slot_ids: {
    [k: string]: unknown;
  };
} | {
  get_wallet_balances: {
    wallet: string;
    [k: string]: unknown;
  };
} | {
  get_state: {
    from_index?: number | null;
    limit?: number | null;
    [k: string]: unknown;
  };
};
export type ValidateIntervalResponse = boolean;