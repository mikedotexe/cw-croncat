#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use croncat_sdk_core::types::{DEFAULT_PAGINATION_FROM_INDEX, DEFAULT_PAGINATION_LIMIT};
use cw_storage_plus::Item;

use cosmwasm_std::{
    to_binary, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Order, Reply, Response, StdResult,
    Storage, SubMsg, WasmMsg,
};
use croncat_sdk_factory::msg::{
    ContractMetadata, ContractMetadataInfo, ContractMetadataResponse, EntryResponse,
    ModuleInstantiateInfo, VersionKind,
};
use cw2::set_contract_version;
use cw_utils::parse_reply_instantiate_data;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    Config, TempReply, CONFIG, CONTRACT_ADDRS, CONTRACT_ADDRS_LOOKUP, CONTRACT_METADATAS,
    LATEST_ADDRS, LATEST_VERSIONS, MAX_URL_LENGTH, TEMP_REPLY,
};

// version info for migration info
const CONTRACT_NAME: &str = "crate:croncat-factory";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Save metadata and generate wasm msg
/// Note: this will override contract metadata if same contract name and version was stored already
fn init_save_metadata_generate_wasm_msg(
    storage: &mut dyn Storage,
    init_info: ModuleInstantiateInfo,
    kind: VersionKind,
    factory: &str,
    funds: Vec<Coin>,
) -> StdResult<WasmMsg> {
    let metadata = ContractMetadata {
        kind,
        code_id: init_info.code_id,
        version: init_info.version,
        commit_id: init_info.commit_id,
        checksum: init_info.checksum,
        changelog_url: init_info.changelog_url,
        schema: init_info.schema,
    };
    CONTRACT_METADATAS.save(
        storage,
        (&init_info.contract_name, &init_info.version),
        &metadata,
    )?;
    LATEST_VERSIONS.save(storage, &init_info.contract_name, &init_info.version)?;

    let msg = WasmMsg::Instantiate {
        admin: Some(factory.to_owned()),
        code_id: init_info.code_id,
        msg: init_info.msg,
        funds,
        // Formats to `CronCat:manager:0.1`
        label: format!(
            "CronCat:{:?}:{:?}.{:?}",
            init_info.contract_name, init_info.version[0], init_info.version[1]
        ),
    };
    Ok(msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let owner_addr = msg
        .owner_addr
        .map(|human| deps.api.addr_validate(&human))
        .transpose()?
        .unwrap_or_else(|| info.sender.clone());
    CONFIG.save(
        deps.storage,
        &Config {
            owner_addr,
            nominated_owner_addr: None,
        },
    )?;

    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    // All factory actions can only be done by the owner_addr
    let config = CONFIG.load(deps.storage)?;
    let is_accept_owner_msg = match msg {
        // Only this method allowed, since we are transitioning to a new owner IF the signer is nominated
        ExecuteMsg::AcceptNominateOwner {} => true,
        _ => false,
    };
    if config.owner_addr != info.sender && !is_accept_owner_msg {
        return Err(ContractError::Unauthorized {});
    }

    match msg {
        ExecuteMsg::Proxy { msg } => execute_proxy(deps, msg),
        ExecuteMsg::Deploy {
            kind,
            module_instantiate_info,
        } => execute_deploy(deps, env, info, kind, module_instantiate_info),
        ExecuteMsg::Remove {
            contract_name,
            version,
        } => execute_remove(deps, contract_name, version),
        ExecuteMsg::UpdateMetadata {
            contract_name,
            version,
            changelog_url,
            schema,
        } => execute_update_metadata(deps, contract_name, version, changelog_url, schema),
        ExecuteMsg::NominateOwner {
            nominated_owner_addr,
        } => execute_nominate_owner(deps, nominated_owner_addr),
        ExecuteMsg::AcceptNominateOwner {} => execute_accept_nominate_owner(deps, info),
        ExecuteMsg::RemoveNominateOwner {} => execute_remove_nominate_owner(deps),
    }
}

fn execute_proxy(deps: DepsMut, msg: WasmMsg) -> Result<Response, ContractError> {
    // Only accept WasmMsg::Execute
    let contract_addr = match &msg {
        WasmMsg::Execute {
            contract_addr,
            funds: _,
            msg: _,
        } => contract_addr,
        // Disallow unknown messages
        _ => {
            return Err(ContractError::UnknownMethod {});
        }
    };

    // Only allow msgs that have existing contract versions
    if !CONTRACT_ADDRS_LOOKUP.has(deps.storage, deps.api.addr_validate(contract_addr)?) {
        return Err(ContractError::UnknownContract {});
    }

    Ok(Response::new()
        .add_attribute("action", "proxy")
        .add_message(msg))
}

fn execute_update_metadata(
    deps: DepsMut,
    contract_name: String,
    version: [u8; 2],
    new_changelog: Option<String>,
    schema: Option<String>,
) -> Result<Response, ContractError> {
    // Validate new_changelog
    check_changelog_length(&new_changelog)?;

    CONTRACT_METADATAS.update(deps.storage, (&contract_name, &version), |metadata_res| {
        match metadata_res {
            Some(mut metadata) => {
                // Update only if it contains values
                // No reason to set it from Some(x) to None
                if new_changelog.is_some() {
                    metadata.changelog_url = new_changelog;
                }
                if schema.is_some() {
                    metadata.schema = schema;
                }
                Ok(metadata)
            }
            None => Err(ContractError::UnknownContract {}),
        }
    })?;
    Ok(Response::new().add_attribute("action", "update_metadata"))
}

fn execute_remove(
    deps: DepsMut,
    contract_name: String,
    version: [u8; 2],
) -> Result<Response, ContractError> {
    let latest_version = LATEST_VERSIONS
        .may_load(deps.storage, &contract_name)?
        .ok_or(ContractError::UnknownContract {})?;
    // Can't remove latest
    if latest_version == version {
        return Err(ContractError::LatestVersionRemove {});
    }

    let metadata = CONTRACT_METADATAS.load(deps.storage, (&contract_name, &version))?;
    let contract_addr = CONTRACT_ADDRS.load(deps.storage, (&contract_name, &version))?;

    // Can't remove unpaused contract if not a library
    if metadata.kind != VersionKind::Library {
        let contract_addr = CONTRACT_ADDRS.load(deps.storage, (&contract_name, &version))?;

        // Check contract pause state, by direct state key
        let pause_state: Item<bool> = Item::new("paused");
        let paused: bool = pause_state.query(&deps.querier, contract_addr)?;
        if !paused {
            return Err(ContractError::NotPaused {});
        }
    }

    CONTRACT_METADATAS.remove(deps.storage, (&contract_name, &version));
    CONTRACT_ADDRS.remove(deps.storage, (&contract_name, &version));
    CONTRACT_ADDRS_LOOKUP.remove(deps.storage, contract_addr);

    Ok(Response::new().add_attribute("action", "remove"))
}

fn execute_deploy(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    kind: VersionKind,
    module_instantiate_info: ModuleInstantiateInfo,
) -> Result<Response, ContractError> {
    // Validate changelog_url
    check_changelog_length(&module_instantiate_info.changelog_url)?;

    if CONTRACT_METADATAS.has(
        deps.storage,
        (
            module_instantiate_info.contract_name.as_str(),
            &module_instantiate_info.version.clone(),
        ),
    ) {
        return Err(ContractError::VersionExists {});
    }

    let contract_name = module_instantiate_info.contract_name.clone();
    let wasm = init_save_metadata_generate_wasm_msg(
        deps.storage,
        module_instantiate_info,
        kind,
        env.contract.address.as_str(),
        info.funds,
    )?;
    let msg = SubMsg::reply_on_success(wasm, 0);

    // Store temporary data that's needed in the reply
    let temp_reply = TempReply { contract_name };
    TEMP_REPLY.save(deps.storage, &temp_reply)?;

    Ok(Response::new()
        .add_attribute("action", "deploy")
        .add_attribute("contract_name", temp_reply.contract_name)
        .add_submessage(msg))
}

/// Proposes a new owner account, can only be transfered if proposed account accepts
fn execute_nominate_owner(
    deps: DepsMut,
    nominated_owner_addr: String,
) -> Result<Response, ContractError> {
    let c = CONFIG.load(deps.storage)?;
    // Nomination shouldn't be current owner
    if c.owner_addr == nominated_owner_addr {
        return Err(ContractError::SameOwnerNominated {});
    }
    let config = Config {
        owner_addr: c.owner_addr,
        nominated_owner_addr: Some(deps.api.addr_validate(&nominated_owner_addr)?),
    };
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "nominate_owner"))
}

/// Nominated account accepts proposal for ownership transfer
fn execute_accept_nominate_owner(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let c = CONFIG.load(deps.storage)?;
    // Nomination shouldn't get accepted from current owner
    if c.owner_addr == info.sender {
        return Err(ContractError::Unauthorized {});
    }
    // Nomination signer should match current nomination
    if let Some(nominated_addr) = c.nominated_owner_addr {
        let nominated = deps.api.addr_validate(nominated_addr.as_str())?;
        if nominated != info.sender {
            return Err(ContractError::Unauthorized {});
        }
        let config = Config {
            // make the transfer
            owner_addr: nominated,
            nominated_owner_addr: None,
        };
        CONFIG.save(deps.storage, &config)?;
    } else {
        return Err(ContractError::Unauthorized {});
    };
    Ok(Response::new().add_attribute("action", "accept_nominate_owner"))
}

/// Current owner removes a valid nomination
fn execute_remove_nominate_owner(deps: DepsMut) -> Result<Response, ContractError> {
    let c = CONFIG.load(deps.storage)?;
    let config = Config {
        owner_addr: c.owner_addr,
        nominated_owner_addr: None,
    };
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "remove_nominate_owner"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::LatestContracts {} => to_binary(&query_latest_contracts(deps)?),
        QueryMsg::LatestContract { contract_name } => {
            to_binary(&query_latest_contract(deps, contract_name)?)
        }
        QueryMsg::VersionsByContractName {
            contract_name,
            from_index,
            limit,
        } => to_binary(&query_versions_by_contract_name(
            deps,
            contract_name,
            from_index,
            limit,
        )?),
        QueryMsg::ContractNames { from_index, limit } => {
            to_binary(&query_contract_names(deps, from_index, limit)?)
        }
        QueryMsg::AllEntries { from_index, limit } => {
            to_binary(&query_all_entries(deps, from_index, limit)?)
        }
    }
}

fn query_all_entries(
    deps: Deps,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<EntryResponse>> {
    let from_index = from_index.unwrap_or(DEFAULT_PAGINATION_FROM_INDEX);
    let limit = limit.unwrap_or(DEFAULT_PAGINATION_LIMIT);
    let metadatas: Vec<((String, Vec<u8>), ContractMetadata)> = CONTRACT_METADATAS
        .range(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .collect::<StdResult<_>>()?;

    let mut entries = Vec::with_capacity(metadatas.len());
    for ((contract_name, version), metadata) in metadatas {
        let contract_addr = CONTRACT_ADDRS.load(deps.storage, (&contract_name, &version))?;
        let metadata_response = ContractMetadataInfo {
            kind: metadata.kind,
            code_id: metadata.code_id,
            contract_addr,
            version: metadata.version,
            commit_id: metadata.commit_id,
            checksum: metadata.checksum,
            changelog_url: metadata.changelog_url,
            schema: metadata.schema,
        };
        let entry = EntryResponse {
            contract_name,
            metadata: metadata_response,
        };
        entries.push(entry);
    }
    Ok(entries)
}

fn query_contract_names(
    deps: Deps,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<String>> {
    let from_index = from_index.unwrap_or(DEFAULT_PAGINATION_FROM_INDEX);
    let limit = limit.unwrap_or(DEFAULT_PAGINATION_LIMIT);
    CONTRACT_ADDRS
        .keys(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .map(|res| res.map(|(contract_name, _)| contract_name))
        .collect()
}

fn query_versions_by_contract_name(
    deps: Deps,
    contract_name: String,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<ContractMetadataInfo>> {
    let from_index = from_index.unwrap_or(DEFAULT_PAGINATION_FROM_INDEX);
    let limit = limit.unwrap_or(DEFAULT_PAGINATION_LIMIT);
    let metadatas: Vec<(Vec<u8>, ContractMetadata)> = CONTRACT_METADATAS
        .prefix(&contract_name)
        .range(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .collect::<StdResult<_>>()?;

    let mut versions = Vec::with_capacity(metadatas.len());
    for (version, metadata) in metadatas {
        let contract_addr = CONTRACT_ADDRS.load(deps.storage, (&contract_name, &version))?;
        let metadata_response = ContractMetadataInfo {
            kind: metadata.kind,
            code_id: metadata.code_id,
            contract_addr,
            version: metadata.version,
            commit_id: metadata.commit_id,
            checksum: metadata.checksum,
            changelog_url: metadata.changelog_url,
            schema: metadata.schema,
        };
        versions.push(metadata_response);
    }
    Ok(versions)
}

pub fn query_latest_contracts(deps: Deps) -> StdResult<Vec<EntryResponse>> {
    let latest_versions: Vec<(String, [u8; 2])> = LATEST_VERSIONS
        .range(deps.storage, None, None, Order::Ascending)
        .collect::<StdResult<_>>()?;
    let mut entries = Vec::with_capacity(latest_versions.len());
    for (contract_name, version) in latest_versions {
        let contract_addr = CONTRACT_ADDRS.load(deps.storage, (&contract_name, &version))?;
        let metadata = CONTRACT_METADATAS.load(deps.storage, (&contract_name, &version))?;
        let metadata_response = ContractMetadataInfo {
            kind: metadata.kind,
            code_id: metadata.code_id,
            contract_addr,
            version: metadata.version,
            commit_id: metadata.commit_id,
            checksum: metadata.checksum,
            changelog_url: metadata.changelog_url,
            schema: metadata.schema,
        };
        let entry = EntryResponse {
            contract_name,
            metadata: metadata_response,
        };
        entries.push(entry);
    }
    Ok(entries)
}

pub fn query_latest_contract(
    deps: Deps,
    contract_name: String,
) -> StdResult<ContractMetadataResponse> {
    let latest_contract_version = LATEST_VERSIONS.may_load(deps.storage, &contract_name)?;
    latest_contract_version
        .map(|version| -> StdResult<_> {
            let contract_addr = CONTRACT_ADDRS.load(deps.storage, (&contract_name, &version))?;
            let metadata = CONTRACT_METADATAS.load(deps.storage, (&contract_name, &version))?;
            Ok(ContractMetadataInfo {
                kind: metadata.kind,
                code_id: metadata.code_id,
                contract_addr,
                version: metadata.version,
                commit_id: metadata.commit_id,
                checksum: metadata.checksum,
                changelog_url: metadata.changelog_url,
                schema: metadata.schema,
            })
        })
        .transpose()
        .map(|op| ContractMetadataResponse { metadata: op })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    let res = parse_reply_instantiate_data(msg)?;

    let contract_name: String = TEMP_REPLY.load(deps.storage)?.contract_name;

    let contract_address = deps.api.addr_validate(&res.contract_address)?;
    LATEST_ADDRS.save(deps.storage, &contract_name, &contract_address)?;

    let latest_version = LATEST_VERSIONS.load(deps.storage, &contract_name)?;

    CONTRACT_ADDRS.save(
        deps.storage,
        (&contract_name, &latest_version),
        &contract_address,
    )?;

    CONTRACT_ADDRS_LOOKUP.save(deps.storage, contract_address, &contract_name)?;

    Ok(Response::new())
}

fn check_changelog_length(changelog_url: &Option<String>) -> Result<(), ContractError> {
    if let Some(url) = changelog_url {
        if url.len() > MAX_URL_LENGTH as usize {
            Err(ContractError::UrlExceededMaxLength {})
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}
