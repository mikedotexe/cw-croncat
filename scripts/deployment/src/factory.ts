import { ExecuteResult, SigningCosmWasmClient, toBinary } from "@cosmjs/cosmwasm-stargate";
import { QueryClient } from "@cosmjs/stargate";
import { GasPrice, StdFee, calculateFee } from "@cosmjs/stargate";
import * as fs from "fs"
import { config } from "dotenv"
import { getContractVersionFromCargoToml } from './utils'
config({ path: '.env' })
const denom: string = process.env.DENOM
const defaultGasPrice = GasPrice.fromString(`0.025${denom}`)
const instantiateGas = calculateFee(700_000, defaultGasPrice)

export class FactoryClient {
	client: SigningCosmWasmClient;
  querier: any;

  constructor(client: SigningCosmWasmClient, querier?: QueryClient) {
		this.client = client;
    this.querier = querier;
	}

	async deploy(
    artifactsRoot: string,
    sender: string,
    uploadGas: StdFee,
    executeGas: StdFee
  ): Promise<[number, string]> {
		const wasm = fs.readFileSync(`${artifactsRoot}/croncat_factory.wasm`)
		const uploadRes = await this.client.upload(sender, wasm, uploadGas)
		const codeId = uploadRes.codeId

    // get the version from cargo
    const version = await getContractVersionFromCargoToml('croncat-factory')

    const instantiateOptions = {
      admin: sender,
      // memo: '',
      // funds: [],
    }

    // instantiate
    const factoryInst = await this.client.instantiate(sender, codeId, {}, `CronCat:factory:${version}`, instantiateGas, instantiateOptions)
    const address = factoryInst.contractAddress

		return [codeId, address];
	}

	async getLatestContracts(contractAddr: string): Promise<any> {
    const q = { latest_contracts: {} };
    const response = await this.querier.wasm.queryContractSmart(contractAddr, q);
		return response;
	}

	async getLatestContract(contractAddr: string, contractName: string): Promise<any> {
    const q = { latest_contract: { contract_name: contractName } };
    const response = await this.querier.wasm.queryContractSmart(contractAddr, q);
		return response;
	}

	async getVersionsByContractName(contractAddr: string, contractName: string): Promise<any> {
    const q = { versions_by_contract_name: { contract_name: contractName } };
    const response = await this.querier.wasm.queryContractSmart(contractAddr, q);
		return response;
  }

  async getContractNames(contractAddr: string): Promise<any> {
    const q = { contract_names: {} };
    const response = await this.querier.wasm.queryContractSmart(contractAddr, q);
    return response;
  }

  async getAllEntries(contractAddr: string): Promise<any> {
    const q = { all_entries: {} };
    const response = await this.querier.wasm.queryContractSmart(contractAddr, q);
    return response;
  }

  async doProxyCall(
    sender: string,
    factoryAddr: string,
    contractAddr: string,
    sub_msg: any,
    gas: StdFee,
    funds: any,
  ): Promise<ExecuteResult> {
    const proxy_msg = {
      proxy: {
        msg: {
          execute: {
            contract_addr: contractAddr || '',
            msg: toBinary(sub_msg),
            funds: funds || [],
          }
        },
      }
    };
    const response = await this.client.execute(sender, factoryAddr, proxy_msg, gas, null, funds);
    return response;
  }

  async addWhitelistedAgent(
    sender: string,
    contractAddr: string,
    agentContractAddr: string,
    agentAddr: string,
    gas: StdFee
  ): Promise<ExecuteResult> {
    const proxy_sub_msg = {
      execute: {
        contract_addr: agentContractAddr || '',
        msg: toBinary({
          add_agent_to_whitelist: {
            agent_address: agentAddr || ''
          }
        }),
        funds: [],
      }
    }
    const proxy_msg = {
      proxy: {
        msg: proxy_sub_msg,
      }
    };
    const response = await this.client.execute(sender, contractAddr, proxy_msg, gas);
    return response;
  }
}