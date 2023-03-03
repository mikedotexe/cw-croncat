use cosmwasm_std::WasmMsg::Migrate;
use cosmwasm_std::{to_binary, Addr, BankMsg, CosmosMsg, Uint128};
use cw_multi_test::{next_block, App, Executor};
use cw_utils::Duration;
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
fn test_dao_single_proposals_with_migration() {
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
    let max_voting_period = Duration::Height(6);
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

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Propose(SingleChoiceProposeMsg {
            title: "Cron".to_string(),
            description: "Cat".to_string(),
            msgs: vec![],
            proposer: None,
        }),
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Propose(SingleChoiceProposeMsg {
            title: "Cron2".to_string(),
            description: "Cat2".to_string(),
            msgs: vec![CosmosMsg::Wasm(Migrate {
                contract_addr: "contract".to_string(),
                new_code_id: 42,
                msg: Default::default(),
            })],
            proposer: None,
        }),
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Propose(SingleChoiceProposeMsg {
            title: "Cron3".to_string(),
            description: "Cat3".to_string(),
            msgs: vec![],
            proposer: None,
        }),
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Propose(SingleChoiceProposeMsg {
            title: "Cron4".to_string(),
            description: "Cat4".to_string(),
            msgs: vec![
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: CREATOR_ADDR.to_string(),
                    amount: vec![],
                }),
                CosmosMsg::Wasm(Migrate {
                    contract_addr: "contract".to_string(),
                    new_code_id: 42,
                    msg: Default::default(),
                }),
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: CREATOR_ADDR.to_string(),
                    amount: vec![],
                }),
            ],
            proposer: None,
        }),
        &[],
    )
    .unwrap();

    // Query passed proposals with migration
    // There aren't any passed proposals yet, so false
    let res: QueryResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::HasPassedProposalWithMigration {
                dao_address: govmod_single.to_string(),
                start_after: Some(0u64),
                limit: Some(101),
            },
        )
        .unwrap();
    assert_eq!(
        res,
        QueryResponse {
            result: false,
            data: to_binary::<std::vec::Vec<u64>>(&vec![]).unwrap(),
        }
    );

    // Approve proposal with migration message
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Vote {
            proposal_id: 2,
            vote: Vote::Yes,
            rationale: None,
        },
        &[],
    )
    .unwrap();

    // Query passed proposals with migration
    // There is one passed proposal with migration with id = 2
    let res: QueryResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::HasPassedProposalWithMigration {
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
            data: to_binary(&vec![2]).unwrap(),
        }
    );

    // Approve proposal without migration message
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            rationale: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Vote {
            proposal_id: 3,
            vote: Vote::Yes,
            rationale: None,
        },
        &[],
    )
    .unwrap();

    // Query passed proposals with migration
    // There is still one passed proposal with migration with id = 2
    let res: QueryResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::HasPassedProposalWithMigration {
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
            data: to_binary(&vec![2]).unwrap(),
        }
    );

    // Approve the last proposal with migration message
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Vote {
            proposal_id: 4,
            vote: Vote::Yes,
            rationale: None,
        },
        &[],
    )
    .unwrap();

    // Query passed proposals with migration
    // Proposals with ids 2 and 4 have Passed status
    let res: QueryResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr,
            &QueryMsg::HasPassedProposalWithMigration {
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
            data: to_binary(&vec![2, 4]).unwrap(),
        }
    );
}

#[test]
fn test_dao_multiple_proposals_with_migration() {
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

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Propose {
            title: "Cron".to_string(),
            description: "Cat".to_string(),
            choices: MultipleChoiceOptions {
                options: vec![
                    MultipleChoiceOption {
                        title: "A".to_string(),
                        description: "a".to_string(),
                        msgs: vec![],
                    },
                    MultipleChoiceOption {
                        title: "B".to_string(),
                        description: "b".to_string(),
                        msgs: vec![],
                    },
                ],
            },
            proposer: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Propose {
            title: "Cron2".to_string(),
            description: "Cat2".to_string(),
            choices: MultipleChoiceOptions {
                options: vec![
                    MultipleChoiceOption {
                        title: "A".to_string(),
                        description: "a".to_string(),
                        msgs: vec![CosmosMsg::Wasm(Migrate {
                            contract_addr: "contract".to_string(),
                            new_code_id: 42,
                            msg: Default::default(),
                        })],
                    },
                    MultipleChoiceOption {
                        title: "B".to_string(),
                        description: "b".to_string(),
                        msgs: vec![],
                    },
                ],
            },
            proposer: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Propose {
            title: "Cron3".to_string(),
            description: "Cat3".to_string(),
            choices: MultipleChoiceOptions {
                options: vec![
                    MultipleChoiceOption {
                        title: "A".to_string(),
                        description: "a".to_string(),
                        msgs: vec![CosmosMsg::Wasm(Migrate {
                            contract_addr: "contract".to_string(),
                            new_code_id: 42,
                            msg: Default::default(),
                        })],
                    },
                    MultipleChoiceOption {
                        title: "B".to_string(),
                        description: "b".to_string(),
                        msgs: vec![],
                    },
                ],
            },
            proposer: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Propose {
            title: "Cron4".to_string(),
            description: "Cat4".to_string(),
            choices: MultipleChoiceOptions {
                options: vec![
                    MultipleChoiceOption {
                        title: "A".to_string(),
                        description: "a".to_string(),
                        msgs: vec![],
                    },
                    MultipleChoiceOption {
                        title: "B".to_string(),
                        description: "b".to_string(),
                        msgs: vec![],
                    },
                ],
            },
            proposer: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Propose {
            title: "Cron5".to_string(),
            description: "Cat5".to_string(),
            choices: MultipleChoiceOptions {
                options: vec![
                    MultipleChoiceOption {
                        title: "A".to_string(),
                        description: "a".to_string(),
                        msgs: vec![
                            CosmosMsg::Bank(BankMsg::Send {
                                to_address: CREATOR_ADDR.to_string(),
                                amount: vec![],
                            }),
                            CosmosMsg::Wasm(Migrate {
                                contract_addr: "contract".to_string(),
                                new_code_id: 42,
                                msg: Default::default(),
                            }),
                            CosmosMsg::Bank(BankMsg::Send {
                                to_address: CREATOR_ADDR.to_string(),
                                amount: vec![],
                            }),
                        ],
                    },
                    MultipleChoiceOption {
                        title: "B".to_string(),
                        description: "b".to_string(),
                        msgs: vec![],
                    },
                ],
            },
            proposer: None,
        },
        &[],
    )
    .unwrap();

    // Query passed proposals with migration
    // There aren't any passed proposals yet, so false
    let res: QueryResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::HasPassedProposalWithMigration {
                dao_address: govmod_single.to_string(),
                start_after: Some(0u64),
                limit: Some(101),
            },
        )
        .unwrap();
    assert_eq!(
        res,
        QueryResponse {
            result: false,
            data: to_binary::<std::vec::Vec<u64>>(&vec![]).unwrap(),
        }
    );

    // Vote on proposal without migration
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
            rationale: Some("because_pulled_pork_mac_n_cheese".to_string()),
        },
        &[],
    )
    .unwrap();

    // Query passed proposals with migration
    // There still aren't any passed proposals yet, so false
    let res: QueryResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::HasPassedProposalWithMigration {
                dao_address: govmod_single.to_string(),
                start_after: Some(0u64),
                limit: Some(101),
            },
        )
        .unwrap();
    assert_eq!(
        res,
        QueryResponse {
            result: false,
            data: to_binary::<std::vec::Vec<u64>>(&vec![]).unwrap(),
        }
    );

    // Vote on proposals 2 and 3
    // Migrate option in the third proposal doesn't pass
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Vote {
            proposal_id: 2,
            vote: MultipleChoiceVote { option_id: 0 },
            rationale: Some("because_pulled_pork_mac_n_cheese".to_string()),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Vote {
            proposal_id: 3,
            vote: MultipleChoiceVote { option_id: 1 },
            rationale: Some("because_pulled_pork_mac_n_cheese".to_string()),
        },
        &[],
    )
    .unwrap();

    // Query passed proposals with migration
    // Only the second proposal has passed and has migration
    let res: QueryResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr.clone(),
            &QueryMsg::HasPassedProposalWithMigration {
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
            data: to_binary(&vec![2]).unwrap(),
        }
    );

    // Vote on proposals 4 and 5
    // Migrate option in the third proposal doesn't pass
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Vote {
            proposal_id: 4,
            vote: MultipleChoiceVote { option_id: 0 },
            rationale: Some("because_pulled_pork_mac_n_cheese".to_string()),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Vote {
            proposal_id: 5,
            vote: MultipleChoiceVote { option_id: 0 },
            rationale: Some("because_pulled_pork_mac_n_cheese".to_string()),
        },
        &[],
    )
    .unwrap();

    // Query passed proposals with migration
    // Now proposals 2 and 5 have passed and have migration
    let res: QueryResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr,
            &QueryMsg::HasPassedProposalWithMigration {
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
            data: to_binary(&vec![2, 5]).unwrap(),
        }
    );
}
