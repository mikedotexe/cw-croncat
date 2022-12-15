<div align="center">
  <h1>
    Cron.cat CW Contracts
  </h1>
  <p>
  The "crontracts" for cosmwasm runtime of croncat service
  </p>
</div>

[![codecov](https://codecov.io/gh/CronCats/cw-croncat/branch/main/graph/badge.svg?token=JU1C4RK7X4)](https://codecov.io/gh/CronCats/cw-croncat)

## ALPHA: 

#### This repo is in early develop stage - use at your own risk!

## Contributing

* [Developing](./Developing.md) how to run tests and develop code. Or go through the
[online tutorial](https://docs.cosmwasm.com/) to get a better feel
of how to develop.
* [Publishing](./Publishing.md) contains useful information on how to publish your contract
to the world, once you are ready to deploy it on a running blockchain.

## Commands
```bash
# Install just
cargo install just

# For building + fmt
just build

# For testing everything
just test

# For schemas
just schema

# Production compilation, run before deploying to live network
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.12.7
```

## Testnet examples

There are some scripts for testnet, mainnet and local node
To build and deploy to testnet:
```bash
just deploy
```

Save the address of the created contract:
```bash
CRONCAT_ADDRESS=juno123somecontract
```

You can create a reccuring task and see it in the list of tasks:
```bash
./create_recurring_task.sh $CRONCAT_ADDRESS user
./get-tasks.sh $CRONCAT_ADDRESS
```

For more examples for registering and unregistering agent, executing task and querying the state see other [scripts](https://github.com/CronCats/cw-croncat/tree/main/contracts/cw-croncat/scripts).

If you want to run croncat manager locally, see [instructions](https://github.com/CronCats/cw-croncat/blob/main/contracts/cw-croncat/scripts/local/README.md) for local setup.

### Agent

```bash
git clone git@github.com:CronCats/croncat-rs.git
cd croncat-rs
```

Before registering an agent modify `config.uni-5.yaml` to include `CRONCAT_ADDRESS` in `contract_address` field.

Create and store new agent address
```bash
cargo run -- --chain-id uni-5 generate-mnemonic --new-name new-agent
AGENT_ADDR=juno123agentaddress
```
Refill `new-agent` balance before using it, so that the agent has some ujunox for `register-agent` and `proxy-call`s.

Register an agent:
```bash
cargo run -- --chain-id uni-5 register-agent --sender-name new-agent payable-account-id
```
Here `payable-account-id` is optional address of the account that receives agent reward.

Start daemon:
```bash
cargo run -- --chain-id uni-5 daemon --sender-name new-agent
```

Unregister the agent:
```bash
cargo run -- --chain-id uni-5 unregister-agent --sender-name new-agent
```
> Default `new-name` and `sender-name` is `agent`.

For other commands see
```bash
cargo run -- help
``` 

## Changelog

### `0.0.1`

Schema Update
```bash
just build
```
Install yarn packages
```bash
yarn --cwd ./typescript install --frozen-lockfile
```

Generate TypeScript files
```bash
yarn --cwd ./typescript codegen
```
