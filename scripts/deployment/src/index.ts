import { setupWasmExtension, SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing"
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
const artifactsRoot = process.env.WASM_BUILD_FOLDER ? `../../${process.env.WASM_BUILD_FOLDER}` : `${process.cwd()}/../../artifacts`

// Gas vals
const uploadGas = calculateFee(4_400_000, defaultGasPrice)
const executeGas = calculateFee(555_000, defaultGasPrice)

const start = async () => {
    console.info(`🤖 Starting Deployment 🤖`)

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

    // Ensure transaction succeeded
    const httpBatchClient = new HttpBatchClient(endpoint, {
        batchSizeLimit: 2,
        dispatchInterval: 500
    })
    const tmClient = await Tendermint34Client.create(httpBatchClient)
    // Keep the line below, as we'll use it later
    // const queryClient = QueryClient.withExtensions(tmClient, setupWasmExtension)

    // Factory
    var factoryClient = new FactoryClient(cwClient);
    var [factoryId, factoryAddress] = await factoryClient.deploy(artifactsRoot, userAddress, uploadGas, executeGas);
    console.info(`🏭 Factory Done`, factoryId, factoryAddress)

    // Manager
    var managerClient = new ManagerClient(cwClient);
    var [managerId, managerAddress] = await managerClient.deploy(
        artifactsRoot,
        userAddress,
        factoryAddress,
        pauseAdminAddress,
        treasuryAddress,
        uploadGas,
        executeGas
    );
    console.info(`🏗️  Manager Done`, managerId, managerAddress)

    // Tasks
    var taskClient = new TaskClient(cwClient);
    var [taskContractCodeId, taskContractAddr] = await taskClient.deploy(
        artifactsRoot,
        userAddress,
        factoryAddress,
        pauseAdminAddress,
        uploadGas,
        executeGas
    );
    console.info(`🏗️  Tasks Done`, taskContractCodeId, taskContractAddr)

    // Agents
    var agentClient = new AgentClient(cwClient);
    var [agentContractCodeId, agentContractAddr] = await agentClient.deploy(
        artifactsRoot,
        userAddress,
        factoryAddress,
        [userAddress, agent2Address],
        pauseAdminAddress,
        uploadGas,
        executeGas
    );
    console.info(`🏗️  Agents Done`, agentContractCodeId, agentContractAddr)

    // Modules
    var modulesClient = new ModulesClient(cwClient);
    var modules = await modulesClient.deploy(artifactsRoot, userAddress, factoryAddress, uploadGas, executeGas);
    console.info(`🏗️  Modules Done`)

    // Show all
    const output = [
        { name: 'factory', code_id: factoryId, address: factoryAddress },
        { name: 'manager', code_id: managerId, address: managerAddress },
        { name: 'agent', code_id: agentContractCodeId, address: agentContractAddr },
        { name: 'tasks', code_id: taskContractCodeId, address: taskContractAddr },
        ...modules,
    ]
    console.table(output)

    // Store this output, for use in agent & website envs
    await fs.writeFileSync(`${artifactsRoot}/${process.env.CHAIN_ID}_deployed_contracts.json`, JSON.stringify(output))

    process.exit()
}

// Start deployment
(() => start())()



