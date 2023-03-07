import { ExecuteResult, SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { Coin, StdFee, QueryClient } from "@cosmjs/stargate";
import * as fs from "fs"
import { config } from "dotenv"
import { getGitHash, getChecksums, getContractVersionFromCargoToml } from './utils'
config({ path: '.env' })
const prefix: string = process.env.PREFIX

export class TaskClient {
  client: SigningCosmWasmClient;
  querier: any;

  constructor(client: SigningCosmWasmClient, querier?: QueryClient) {
    this.client = client;
    this.querier = querier;
  }

  async deploy(
    artifactsRoot: string,
    sender: string,
    factoryAddress: string,
    pauserAddress: string,
    uploadGas: StdFee,
    executeGas: StdFee
  ): Promise<[number, string]> {
    const wasm = fs.readFileSync(`${artifactsRoot}/croncat_tasks.wasm`)
    const uploadRes = await this.client.upload(sender, wasm, uploadGas)
    const codeId = uploadRes.codeId
    const checksums = await getChecksums()
    const githash = await getGitHash()

    // get the version from cargo
    const version = await getContractVersionFromCargoToml('croncat-tasks')

    // instantiate manager contract (from the factory)
    const deployMsg = {
      "deploy": {
        "kind": "tasks",
        "module_instantiate_info": {
          "code_id": codeId,
          "version": version,
          "commit_id": githash || '-',
          "checksum": checksums.tasks || '-',
          "changelog_url": "https://github.com/croncats",
          "schema": "",
          "msg": Buffer.from(JSON.stringify({
            chain_name: prefix || 'juno',
            version: `${version[0]}.${version[1]}`,
            pause_admin: `${pauserAddress}`,
            croncat_manager_key: ['manager', version || [0, 1]],
            croncat_agents_key: ['agents', version || [0, 1]],
            // slot_granularity_time: '',
            // gas_base_fee: '',
            // gas_action_fee: '',
            // gas_query_fee: '',
            // gas_limit: '',
          })).toString('base64'),
          "contract_name": "tasks"
        }
      }
    }
    const instRes = await this.client.execute(sender, factoryAddress, deployMsg, executeGas);
    const address: string = instRes.logs[0].events[1].attributes[0].value

    return [codeId, address];
  }

  async getTasks(contractAddr: string): Promise<any> {
    const q = { tasks: {} };
    // const q = { tasks_with_queries: {} };
    const response = await this.querier.wasm.queryContractSmart(contractAddr, q);
    return response;
  }

  async create(sender: string, contractAddr: string, gas: StdFee, task: any, funds: Coin[]): Promise<ExecuteResult> {
    const msg = { create_task: { task } };
    const response = await this.client.execute(sender, contractAddr, msg, gas, undefined, funds);
    return response;
  }

  async remove(sender: string, contractAddr: string, gas: StdFee, task_hash: any): Promise<ExecuteResult> {
    const msg = { remove_task: { task_hash } };
    const response = await this.client.execute(sender, contractAddr, msg, gas);
    return response;
  }
}
