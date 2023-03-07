import { setupWasmExtension, SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { coins, DirectSecp256k1HdWallet } from "@cosmjs/proto-signing"
import { HdPath, stringToPath } from "@cosmjs/crypto"
import { HttpBatchClient, Tendermint34Client, TxResponse } from "@cosmjs/tendermint-rpc"
import { QueryClient } from "@cosmjs/stargate";
import { fromHex } from "@cosmjs/encoding";
import { config } from "dotenv"
import { GasPrice, StdFee, calculateFee } from "@cosmjs/stargate"
import * as fs from "fs"
import * as util from "util"
import { FactoryClient } from './factory';
import { ManagerClient } from './manager';
import { TaskClient } from './tasks';
import { AgentClient } from './agents';
import { ModulesClient } from './modules';
config({ path: '.env' })
// Get values from the environment variables located in the .env file
const seedPhrase: string = process.env.SEED_PHRASE
const prefix: string = process.env.PREFIX
const endpoint: string = process.env.RPC_ENDPOINT
// NOTE: MUST Be a contract wallet - multisig prefered!
// If you need one, go to https://github.com/CosmWasm/cw-plus/tree/main/contracts/cw3-fixed-multisig, compile, instantiate & get deployed address.
const pauseAdminAddress: string = process.env.PAUSE_ADMIN_MULTISIG || ''
const denom: string = process.env.DENOM
const defaultGasPrice = GasPrice.fromString(`0.025${denom}`)
const artifactsRoot = `${process.cwd()}/../../artifacts`

// Gas vals
const executeGas = calculateFee(999_000, defaultGasPrice)

const start = async () => {
	console.info(`🏖️ Starting End 2 End Chex 🌋`)

	const signerWallet = await DirectSecp256k1HdWallet.fromMnemonic(seedPhrase, {
		prefix,
		hdPaths: [
			// for easier coinage management
			stringToPath(`m/44'/118'/0'/0/0`),
			stringToPath(`m/44'/118'/0'/0/1`),
			stringToPath(`m/44'/118'/0'/0/2`),
			stringToPath(`m/44'/118'/0'/0/3`),
			stringToPath(`m/44'/118'/0'/0/4`),
		]
	})
	const accts = await signerWallet.getAccounts()
	const userAddress = accts[0].address
	const agent2Address = accts[1].address
	const agent3Address = accts[2].address
	const agent4Address = accts[3].address
	const treasuryAddress = accts[4].address
	console.table({
		userAddress,
		agent2Address,
		agent3Address,
		agent4Address,
		treasuryAddress,
		pauseAdminAddress,
	});

	const cwClient = await SigningCosmWasmClient.connectWithSigner(endpoint, signerWallet, { prefix, gasPrice: defaultGasPrice })
	const httpBatchClient = new HttpBatchClient(endpoint, {
			batchSizeLimit: 2,
			dispatchInterval: 500
	})
	const tmClient = await Tendermint34Client.create(httpBatchClient)
	const queryClient = QueryClient.withExtensions(tmClient, setupWasmExtension)
	const rawDeployed = fs.readFileSync(`${artifactsRoot}/${process.env.CHAIN_ID}_deployed_contracts.json`, 'utf8')
	if (!rawDeployed) process.exit(1)
	const deployedContracts = JSON.parse(rawDeployed)
	const contracts: any = {}
	deployedContracts.forEach(d => {
		// create a map instead of array
		contracts[d.name] = { codeId: d.code_id, address: d.address }
	})

	// Classes
	const factoryClient = new FactoryClient(cwClient, queryClient);
	const managerClient = new ManagerClient(cwClient);
	const agentClient = new AgentClient(cwClient, queryClient);
	const taskClient = new TaskClient(cwClient, queryClient);
	// NOTE: Unsure if we really need module thangs here. maybe someday when haz too much hands and excessive timez

	// Pre-logic: get latest versions from factory
	const allVersions: any[] = await factoryClient.getLatestContracts(contracts.factory.address)
	const versions: any = {}
	allVersions.forEach((v: any) => {
		// create a map instead of array
		versions[v.contract_name] = v.metadata
	})
	// console.log('factory allVersions', JSON.stringify(allVersions));

	// TODO: Replace with better example
	// const task1 = {
	// 	"actions": [
	// 		{
	// 			"msg": {
	// 				"wasm": {
	// 					"execute": {
	// 						"contract_addr": versions.manager.contract_addr,
	// 						"msg": Buffer.from(JSON.stringify({ "tick": {} })).toString('base64'),
	// 						"funds": []
	// 					}
	// 				}
	// 			},
	// 			"gas_limit": 75000
	// 		}
	// 	],
	// 	"boundary": null,
	// 	"cw20": null,
	// 	"interval": {
	// 		"block": 1
	// 	},
	// 	"stop_on_fail": true,
	// 	"queries": null,
	// 	"transforms": null
	// }
	const task2 = (amount: number) => ({
		"actions": [
			{
				"msg": {
					"bank": {
						"send": {
							"to_address": versions.manager.contract_addr,
							"amount": coins(amount, denom)
						}
					}
				},
				"gas_limit": 75000
			}
		],
		"boundary": null,
		"cw20": null,
		"interval": {
			"block": 1
		},
		"stop_on_fail": false,
		"queries": null,
		"transforms": null
	})

	const factoryTask1 = {
		"create_task": {
			"task": {
				"actions": [
					{
						"msg": {
							"wasm": {
								"execute": {
									"contract_addr": versions.agents.contract_addr,
									"msg": Buffer.from(JSON.stringify({ "tick": {} })).toString('base64'),
									"funds": []
								}
							}
						},
						"gas_limit": 75000
					}
				],
				"boundary": null,
				"cw20": null,
				"interval": {
					"block": 1
				},
				"stop_on_fail": true,
				"queries": null,
				"transforms": null
			}
		}
	}

	// Add first agent Register & check status
	try {
		const r = await factoryClient.addWhitelistedAgent(
			userAddress,
			contracts.factory.address,
			versions.agents.contract_addr,
			userAddress,
			executeGas
		);
		console.info(`Agents Add to Whitelist SUCCESS\n`, JSON.stringify(r), '\n')
	} catch (e) {
		console.info(`Agents Add to Whitelist ERROR`, e)
	}

	// Register & check status
	try {
		const r = await agentClient.register(userAddress, versions.agents.contract_addr, executeGas);
		console.info(`Agents Register SUCCESS\n`, JSON.stringify(r), '\n')
	} catch (e) {
		console.info(`Agents Register ERROR`, e)
	}
	try {
		const as = await agentClient.status(userAddress, versions.agents.contract_addr);
		console.info(`Agents Status\n`, as.agent, '\n')
		if (as.agent.status !== 'active') process.exit(1)
	} catch (e) {
		console.info(`Agents Status ERROR`, e)
	}

	// Create 2 tasks (first one, the stock factory tick task)
	try {
		const t1 = await factoryClient.doProxyCall(
			userAddress,
			contracts.factory.address,
			versions.tasks.contract_addr,
			factoryTask1,
			executeGas,
			coins(60_000, denom)
		);
		console.info(`Factory Task 1 Create SUCCESS\n`, JSON.stringify(t1), '\n')
	} catch (e) {
		console.info(`Factory Task 1 Create ERROR`, e)
	}
	// TODO: Bring back once better example above
	// try {
	// 	const t1 = await taskClient.create(userAddress, versions.tasks.contract_addr, executeGas, task1, coins(60_000, denom));
	// 	console.info(`Task 1 Create SUCCESS\n`, JSON.stringify(t1), '\n')
	// } catch (e) {
	// 	console.info(`Task 1 Create ERROR`, e)
	// }
	try {
		const t2 = await taskClient.create(userAddress, versions.tasks.contract_addr, executeGas, task2(1), coins(100_000, denom));
		console.info(`Task 2 Create SUCCESS\n`, JSON.stringify(t2), '\n')
	} catch (e) {
		console.info(`Task 2 Create ERROR`, e)
	}

	// NOTE: Need to fund this address to work
	// Send agent 2 small funds to execute sample tasks
	try {
		await cwClient.sendTokens(userAddress, agent2Address, coins(5_000_000, denom), "auto", "CronCat Agent 2")
	} catch (e) {
		console.log('Fund Agent 2 ERROR', e);
		process.exit(1)
	}

	// Register 2nd agent & check status
	try {
		const r2 = await agentClient.register(agent2Address, versions.agents.contract_addr, executeGas);
		console.info(`Agents Register SUCCESS\n`, JSON.stringify(r2), '\n')
	} catch (e) {
		console.info(`Agents Register ERROR`, e)
	}
	try {
		const as2 = await agentClient.status(agent2Address, versions.agents.contract_addr);
		console.info(`Agent 2 Status\n`, as2.agent, '\n')
		if (as2.agent.status !== 'pending') process.exit(1)
	} catch (e) {
		console.info(`Agent 2 Status ERROR`, e)
	}

	// create another 2 tasks so second agent can be nominated
	try {
		const t3 = await taskClient.create(userAddress, versions.tasks.contract_addr, executeGas, task2(2), coins(260_000, denom));
		console.info(`Task 3 Create SUCCESS\n`, JSON.stringify(t3), '\n')
	} catch (e) {
		console.info(`Task 3 Create ERROR`, e)
	}
	try {
		const t4 = await taskClient.create(userAddress, versions.tasks.contract_addr, executeGas, task2(3), coins(460_000, denom));
		console.info(`Task 3 Create SUCCESS\n`, JSON.stringify(t4), '\n')
	} catch (e) {
		console.info(`Task 3 Create ERROR`, e)
	}

	// confirm agent is nominated
	try {
		const as2 = await agentClient.status(agent2Address, versions.agents.contract_addr);
		console.info(`Agent 2 Nominated`, as2.agent.status)
		if (as2.agent.status !== 'nominated') process.exit(1)
	} catch (e) {
		console.info(`Agent 2 Nominated ERROR`, e)
	}

	// Check in 2nd agent
	try {
		const as2 = await agentClient.checkIn(agent2Address, versions.agents.contract_addr, executeGas);
		console.info(`Agent 2 Checkin\n`, JSON.stringify(as2), '\n')
	} catch (e) {
		console.info(`Agent 2 Nominated ERROR`, e)
	}
	// confirm agent is active
	try {
		const as2 = await agentClient.status(agent2Address, versions.agents.contract_addr);
		console.info(`Agent 2 Active`, as2.agent.status)
		if (as2.agent.status !== 'active') process.exit(1)
	} catch (e) {
		console.info(`Agent 2 Activated ERROR`, e)
	}

	// 1st agent do proxycall
	try {
		const a1pc = await managerClient.proxyCall(userAddress, versions.manager.contract_addr, executeGas);
		console.info(`Agent 1 ProxyCall\n`, JSON.stringify(a1pc), '\n')
	} catch (e) {
		console.info(`Agent 1 ProxyCall ERROR`, e)
	}

	// 2nd agent do proxycall
	try {
		const a2pc = await managerClient.proxyCall(agent2Address, versions.manager.contract_addr, executeGas);
		console.info(`Agent 2 ProxyCall\n`, JSON.stringify(a2pc), '\n')
	} catch (e) {
		console.info(`Agent 2 ProxyCall ERROR`, e)
	}

	// 1st agent withdraw reward
	try {
		const a1w = await managerClient.agentWithdraw(userAddress, versions.manager.contract_addr, executeGas);
		console.info(`Agent 1 Withdraw\n`, JSON.stringify(a1w), '\n')
	} catch (e) {
		console.info(`Agent 1 Withdraw ERROR`, e)
	}

	// 1st agent unregister
	try {
		const as1u = await agentClient.unregister(userAddress, versions.agents.contract_addr, executeGas);
		console.info(`Agent 1 Unregister\n`, JSON.stringify(as1u), '\n')
	} catch (e) {
		console.info(`Agent 1 Unregister ERROR`, e)
	}

	// 2nd agent unregister
	try {
		const as2u = await agentClient.unregister(agent2Address, versions.agents.contract_addr, executeGas);
		console.info(`Agent 2 Unregister\n`, JSON.stringify(as2u), '\n')
	} catch (e) {
		console.info(`Agent 2 Unregister ERROR`, e)
	}

	// Confirm no agents
	try {
		const aIds = await agentClient.getAgents(versions.agents.contract_addr);
		console.info(`Agents List Empty`, aIds)
		if (aIds.active.length > 0 || aIds.pending.length > 0) process.exit(1)
	} catch (e) {
		console.info(`Agents List Empty ERROR`, e)
	}

	// Get list of all tasks
	let tasks = []
	try {
		const t = await taskClient.getTasks(versions.tasks.contract_addr);
		console.info(`Tasks`, t.length)
		if (t.length <= 0) process.exit(1)
		tasks = t
	} catch (e) {
		console.info(`Tasks List ERROR`, e)
	}

	// Loop and remove all tasks
	for await (const task of tasks) {
		try {
			const t = await taskClient.remove(task.owner_addr, versions.tasks.contract_addr, executeGas, task.task_hash);
			console.info(`Task Remove SUCCESS\n`, task.task_hash, '\n', JSON.stringify(t), '\n')
		} catch (e) {
			console.info(`Task Remove ERROR`, e)
		}
	}

	process.exit()
}

// Start deployment
(() => start())()
