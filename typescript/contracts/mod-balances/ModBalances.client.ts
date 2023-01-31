/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.19.0.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

import { CosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { InstantiateMsg, ExecuteMsg, QueryMsg, BalanceComparator, Balance, Uint128, NativeBalance, Addr, HasBalanceComparator, Coin, Cw20CoinVerified, Binary, QueryResponseForBinary } from "./ModBalances.types";
export interface ModBalancesReadOnlyInterface {
  contractAddress: string;
  getBalance: ({
    address,
    denom
  }: {
    address: string;
    denom: string;
  }) => Promise<QueryResponseForBinary>;
  getCw20Balance: ({
    address,
    cw20Contract
  }: {
    address: string;
    cw20Contract: string;
  }) => Promise<QueryResponseForBinary>;
  hasBalanceComparator: ({
    address,
    comparator,
    requiredBalance
  }: {
    address: string;
    comparator: BalanceComparator;
    requiredBalance: Balance;
  }) => Promise<QueryResponseForBinary>;
}
export class ModBalancesQueryClient implements ModBalancesReadOnlyInterface {
  client: CosmWasmClient;
  contractAddress: string;

  constructor(client: CosmWasmClient, contractAddress: string) {
    this.client = client;
    this.contractAddress = contractAddress;
    this.getBalance = this.getBalance.bind(this);
    this.getCw20Balance = this.getCw20Balance.bind(this);
    this.hasBalanceComparator = this.hasBalanceComparator.bind(this);
  }

  getBalance = async ({
    address,
    denom
  }: {
    address: string;
    denom: string;
  }): Promise<QueryResponseForBinary> => {
    return this.client.queryContractSmart(this.contractAddress, {
      get_balance: {
        address,
        denom
      }
    });
  };
  getCw20Balance = async ({
    address,
    cw20Contract
  }: {
    address: string;
    cw20Contract: string;
  }): Promise<QueryResponseForBinary> => {
    return this.client.queryContractSmart(this.contractAddress, {
      get_cw20_balance: {
        address,
        cw20_contract: cw20Contract
      }
    });
  };
  hasBalanceComparator = async ({
    address,
    comparator,
    requiredBalance
  }: {
    address: string;
    comparator: BalanceComparator;
    requiredBalance: Balance;
  }): Promise<QueryResponseForBinary> => {
    return this.client.queryContractSmart(this.contractAddress, {
      has_balance_comparator: {
        address,
        comparator,
        required_balance: requiredBalance
      }
    });
  };
}