use cosmwasm_std::{to_binary, Addr, Binary, Uint128};
use cw_multi_test::{next_block, App, Executor};
use dao_core::state::ProposalModule;
use dao_voting::{
    multiple_choice::{
        MultipleChoiceOption, MultipleChoiceOptions, MultipleChoiceVote, VotingStrategy,
    },
    proposal::SingleChoiceProposeMsg,
    threshold::{PercentageThreshold, Threshold},
    voting::Vote,
};
use mod_sdk::types::QueryResponse;

use crate::{
    msg::{InstantiateMsg, QueryMsg},
    tests::helpers::{
        contract_template, instantiate_with_staking_active_threshold, multiple_proposal_contract,
        single_proposal_contract, CREATOR_ADDR, VERSION,
    },
};

#[test]
fn test_single_check_passed_proposals() {
    let mut app = App::default();
    let code_id = app.store_code(contract_template());

    let instantiate = InstantiateMsg {
        version: Some(VERSION.to_owned()),
    };
    let contract_addr = app
        .instantiate_contract(
            code_id,
            Addr::unchecked(CREATOR_ADDR),
            &instantiate,
            &[],
            "cw-rules",
            None,
        )
        .unwrap();

    let proposal_module_code_id = app.store_code(single_proposal_contract());
    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate_govmod = dao_proposal_single::msg::InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        close_proposal_on_execution_failure: true,
        pre_propose_info: dao_voting::pre_propose::PreProposeInfo::AnyoneMayPropose {},
    };
    let governance_addr = instantiate_with_staking_active_threshold(
        &mut app,
        proposal_module_code_id,
        to_binary(&instantiate_govmod).unwrap(),
        None,
        None,
    );
    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &dao_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: dao_proposal_single::state::Config = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_single::msg::QueryMsg::Config {},
        )
        .unwrap();
    let dao = govmod_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &dao_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &dao_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Stake some tokens so we can propose
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(2000),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), token_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Create 100 new proposals
    for num in 1..101 {
        let mut title = "Cron".to_string();
        title.push(num.into());
        let mut description = "Cat".to_string();
        description.push(num.into());
        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &dao_proposal_single::msg::ExecuteMsg::Propose(SingleChoiceProposeMsg {
                title,
                description,
                msgs: vec![],
                proposer: None,
            }),
            &[],
        )
        .unwrap();
    }

    // Neither proposal has passed
    let res: QueryResponse<Binary> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::HasPassedProposals {
                dao_address: govmod_single.to_string(),
                start_after: Some(0u64),
                limit: Some(101),
            },
        )
        .unwrap();
    assert!(!res.result);

    // Approve even proposals
    for num in 1..51 {
        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &dao_proposal_single::msg::ExecuteMsg::Vote {
                proposal_id: 2 * num as u64,
                vote: Vote::Yes,
                rationale: None,
            },
            &[],
        )
        .unwrap();
    }

    // Query passed proposals and execute them
    for num in 1..51 {
        let index: u64 = 2 * num;

        // Check that CheckPassedProposals returns index as the number of the first passed proposal
        let res: QueryResponse<Binary> = app
            .wrap()
            .query_wasm_smart(
                contract_addr.clone(),
                &QueryMsg::HasPassedProposals {
                    dao_address: govmod_single.to_string(),
                    start_after: Some(0u64),
                    limit: Some(101),
                },
            )
            .unwrap();
        assert_eq!(
            res,
            QueryResponse {
                result: true,
                data: to_binary(&index).unwrap()
            }
        );

        // Execute the proposal
        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &dao_proposal_single::msg::ExecuteMsg::Execute { proposal_id: index },
            &[],
        )
        .unwrap();
    }

    // There're no passed proposals
    let res: QueryResponse<Binary> = app
        .wrap()
        .query_wasm_smart(
            contract_addr,
            &QueryMsg::HasPassedProposals {
                dao_address: govmod_single.to_string(),
                start_after: Some(0u64),
                limit: Some(101),
            },
        )
        .unwrap();
    assert!(!res.result);
}

#[test]
fn test_multiple_check_passed_proposals() {
    let mut app = App::default();
    let code_id = app.store_code(contract_template());
    let instantiate = InstantiateMsg {
        version: Some(VERSION.to_owned()),
    };
    let contract_addr = app
        .instantiate_contract(
            code_id,
            Addr::unchecked(CREATOR_ADDR),
            &instantiate,
            &[],
            "cw-rules",
            None,
        )
        .unwrap();

    let proposal_module_code_id = app.store_code(multiple_proposal_contract());
    let voting_strategy = VotingStrategy::SingleChoice {
        quorum: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate_govmod = dao_proposal_multiple::msg::InstantiateMsg {
        voting_strategy,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        close_proposal_on_execution_failure: true,
        pre_propose_info: dao_voting::pre_propose::PreProposeInfo::AnyoneMayPropose {},
    };
    let governance_addr = instantiate_with_staking_active_threshold(
        &mut app,
        proposal_module_code_id,
        to_binary(&instantiate_govmod).unwrap(),
        None,
        None,
    );
    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &dao_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: dao_proposal_multiple::state::Config = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_multiple::msg::QueryMsg::Config {},
        )
        .unwrap();
    let dao = govmod_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &dao_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &dao_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Stake some tokens so we can propose
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(2000),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), token_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Create 100 new proposals
    for num in 1..101 {
        let mut title = "Cron".to_string();
        title.push(num.into());
        let mut description = "Cat".to_string();
        description.push(num.into());
        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &dao_proposal_multiple::msg::ExecuteMsg::Propose {
                title,
                description,
                choices: MultipleChoiceOptions {
                    options: vec![
                        MultipleChoiceOption {
                            description: "a".to_string(),
                            title: "A".to_string(),
                            msgs: vec![],
                        },
                        MultipleChoiceOption {
                            description: "b".to_string(),
                            title: "B".to_string(),
                            msgs: vec![],
                        },
                    ],
                },
                proposer: None,
            },
            &[],
        )
        .unwrap();
    }

    // Neither proposal has passed
    let res: QueryResponse<Binary> = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::HasPassedProposals {
                dao_address: govmod_single.to_string(),
                start_after: Some(0u64),
                limit: Some(101),
            },
        )
        .unwrap();
    assert!(!res.result,);

    // Vote on even proposals
    for num in 1..51 {
        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &dao_proposal_multiple::msg::ExecuteMsg::Vote {
                proposal_id: 2 * num as u64,
                vote: MultipleChoiceVote { option_id: 0 },
                rationale: Some("because_pulled_pork_mac_n_cheese".to_string()),
            },
            &[],
        )
        .unwrap();
    }

    // Query passed proposals and execute them
    for num in 1..51 {
        let index: u64 = 2 * num;

        // Check that CheckPassedProposals returns index as the number of the first passed proposal
        let res: QueryResponse<Binary> = app
            .wrap()
            .query_wasm_smart(
                contract_addr.clone(),
                &QueryMsg::HasPassedProposals {
                    dao_address: govmod_single.to_string(),
                    start_after: Some(0u64),
                    limit: Some(101),
                },
            )
            .unwrap();

        // Execute the proposal
        assert_eq!(
            res,
            QueryResponse {
                result: true,
                data: to_binary(&index).unwrap()
            }
        );

        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &dao_proposal_multiple::msg::ExecuteMsg::Execute { proposal_id: index },
            &[],
        )
        .unwrap();
    }

    // There're no passed proposals
    let res: QueryResponse<Binary> = app
        .wrap()
        .query_wasm_smart(
            contract_addr,
            &QueryMsg::HasPassedProposals {
                dao_address: govmod_single.to_string(),
                start_after: Some(0u64),
                limit: Some(101),
            },
        )
        .unwrap();
    assert!(!res.result,);
}
