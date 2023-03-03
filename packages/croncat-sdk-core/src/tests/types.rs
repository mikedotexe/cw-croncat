use cosmwasm_std::{coin, Addr};
use cw20::Cw20CoinVerified;

use crate::error::SdkError;
use crate::types::{AmountForOneTask, GasPrice};

#[test]
fn failed_gas_calculations() {
    let gas_price_wrapper = GasPrice::default();

    let err = gas_price_wrapper.calculate(u64::MAX).unwrap_err();
    assert!(matches!(err, SdkError::InvalidGas {}));
}

#[test]
fn amount_for_one_task_add_gas() {
    let mut amount = AmountForOneTask {
        cw20: None,
        coin: [None, None],
        gas: 0,
        agent_fee: u16::default(),
        treasury_fee: u16::default(),
        gas_price: GasPrice::default(),
    };

    amount.add_gas(10);
    assert_eq!(amount.gas, 10);

    amount.add_gas(5);
    assert_eq!(amount.gas, 15);

    amount.add_gas(u64::MAX);
    assert_eq!(amount.gas, u64::MAX);
}

#[test]
fn amount_for_one_task_add_coin() {
    let mut amount = AmountForOneTask {
        cw20: None,
        coin: [None, None],
        gas: 0,
        agent_fee: u16::default(),
        treasury_fee: u16::default(),
        gas_price: GasPrice::default(),
    };

    // Add the first coin
    let mut coin1 = coin(10, "denom1".to_string());
    assert!(amount.add_coin(coin1.clone()).unwrap());
    assert_eq!(amount.coin, [Some(coin1.clone()), None]);

    // Add the second coin
    let mut coin2 = coin(100, "denom2".to_string());
    assert!(amount.add_coin(coin2.clone()).unwrap());
    assert_eq!(amount.coin, [Some(coin1), Some(coin2.clone())]);

    // Add coin with the first denom
    coin1 = coin(20, "denom1".to_string());
    assert!(amount.add_coin(coin1).unwrap());
    assert_eq!(
        amount.coin,
        [Some(coin(30, "denom1".to_string())), Some(coin2.clone())]
    );

    // Add coin with the second denom
    coin2 = coin(200, "denom2".to_string());
    assert!(amount.add_coin(coin2).unwrap());
    assert_eq!(
        amount.coin,
        [
            Some(coin(30, "denom1".to_string())),
            Some(coin(300, "denom2".to_string()))
        ]
    );

    // Add coin with a new denom, return false
    let another_coin = coin(1, "denom3".to_string());
    assert!(!amount.add_coin(another_coin).unwrap());
    assert_eq!(
        amount.coin,
        [
            Some(coin(30, "denom1".to_string())),
            Some(coin(300, "denom2".to_string()))
        ]
    );
}

#[test]
fn amount_for_one_task_add_cw20() {
    let mut amount = AmountForOneTask {
        cw20: None,
        coin: [None, None],
        gas: 0,
        agent_fee: u16::default(),
        treasury_fee: u16::default(),
        gas_price: GasPrice::default(),
    };

    // Add cw20 coin
    let mut cw20 = Cw20CoinVerified {
        address: Addr::unchecked("addr"),
        amount: 1u64.into(),
    };
    assert!(amount.add_cw20(cw20.clone()));
    assert_eq!(amount.cw20, Some(cw20));

    // Add cw20 coin with the same address
    cw20 = Cw20CoinVerified {
        address: Addr::unchecked("addr"),
        amount: 10u64.into(),
    };
    assert!(amount.add_cw20(cw20.clone()));
    assert_eq!(
        amount.cw20,
        Some(Cw20CoinVerified {
            address: Addr::unchecked("addr"),
            amount: 11u64.into(),
        })
    );

    // Add cw20 coin with a wrong address
    cw20 = Cw20CoinVerified {
        address: Addr::unchecked("addr2"),
        amount: 10u64.into(),
    };
    assert!(!amount.add_cw20(cw20));
    assert_eq!(
        amount.cw20,
        Some(Cw20CoinVerified {
            address: Addr::unchecked("addr"),
            amount: 11u64.into(),
        })
    );
}

#[test]
fn amount_for_one_task_sub_coin() {
    let mut amount = AmountForOneTask {
        cw20: None,
        coin: [None, None],
        gas: 0,
        agent_fee: u16::default(),
        treasury_fee: u16::default(),
        gas_price: GasPrice::default(),
    };

    let coin1 = coin(10, "denom1".to_string());
    assert!(amount.sub_coin(&coin1).is_err());
    assert_eq!(
        amount,
        AmountForOneTask {
            cw20: None,
            coin: [None, None],
            gas: 0,
            agent_fee: u16::default(),
            treasury_fee: u16::default(),
            gas_price: GasPrice::default(),
        }
    );

    // Add the first coin
    assert!(amount.add_coin(coin1.clone()).unwrap());
    assert_eq!(amount.coin, [Some(coin1.clone()), None]);

    // Check sub_coin when amount already contains one coin
    amount.sub_coin(&coin(1, "denom1".to_string())).unwrap();
    assert_eq!(amount.coin, [Some(coin(9, "denom1".to_string())), None]);

    assert!(amount.sub_coin(&coin(10, "denom1".to_string())).is_err());
    assert_eq!(amount.coin, [Some(coin(9, "denom1".to_string())), None]);

    assert!(amount.sub_coin(&coin(1, "denom2".to_string())).is_err());
    assert_eq!(amount.coin, [Some(coin(9, "denom1".to_string())), None]);

    // Add the second coin
    let coin2 = coin(100, "denom2".to_string());
    assert!(amount.add_coin(coin2.clone()).unwrap());
    assert_eq!(
        amount.coin,
        [Some(coin(9, "denom1".to_string())), Some(coin2.clone())]
    );

    // Check sub_coin when amount already has two coins
    amount.sub_coin(&coin(2, "denom1".to_string())).unwrap();
    assert_eq!(
        amount.coin,
        [Some(coin(7, "denom1".to_string())), Some(coin2)]
    );

    amount.sub_coin(&coin(10, "denom2".to_string())).unwrap();
    assert_eq!(
        amount.coin,
        [
            Some(coin(7, "denom1".to_string())),
            Some(coin(90, "denom2".to_string()))
        ]
    );

    assert!(amount.sub_coin(&coin(8, "denom1".to_string())).is_err());
    assert_eq!(
        amount.coin,
        [
            Some(coin(7, "denom1".to_string())),
            Some(coin(90, "denom2".to_string()))
        ]
    );

    assert!(amount.sub_coin(&coin(91, "denom2".to_string())).is_err());
    assert_eq!(
        amount.coin,
        [
            Some(coin(7, "denom1".to_string())),
            Some(coin(90, "denom2".to_string()))
        ]
    );

    assert!(amount.sub_coin(&coin(1, "denom3".to_string())).is_err());
    assert_eq!(
        amount.coin,
        [
            Some(coin(7, "denom1".to_string())),
            Some(coin(90, "denom2".to_string()))
        ]
    );
}

#[test]
fn amount_for_one_task_sub_cw20() {
    let mut amount = AmountForOneTask {
        cw20: None,
        coin: [None, None],
        gas: 0,
        agent_fee: u16::default(),
        treasury_fee: u16::default(),
        gas_price: GasPrice::default(),
    };

    let cw20 = Cw20CoinVerified {
        address: Addr::unchecked("addr"),
        amount: 10u64.into(),
    };
    assert!(amount.sub_cw20(&cw20).is_err());

    // Add cw20 coin
    assert!(amount.add_cw20(cw20.clone()));
    assert_eq!(amount.cw20, Some(cw20));

    // Check sub_cw20
    amount
        .sub_cw20(&Cw20CoinVerified {
            address: Addr::unchecked("addr"),
            amount: 1u64.into(),
        })
        .unwrap();
    assert_eq!(
        amount.cw20,
        Some(Cw20CoinVerified {
            address: Addr::unchecked("addr"),
            amount: 9u64.into(),
        })
    );

    assert!(amount
        .sub_cw20(&Cw20CoinVerified {
            address: Addr::unchecked("addr"),
            amount: 10u64.into(),
        })
        .is_err());
    assert_eq!(
        amount.cw20,
        Some(Cw20CoinVerified {
            address: Addr::unchecked("addr"),
            amount: 9u64.into(),
        })
    );

    assert!(amount
        .sub_cw20(&Cw20CoinVerified {
            address: Addr::unchecked("addr2"),
            amount: 1u64.into(),
        })
        .is_err());
    assert_eq!(
        amount.cw20,
        Some(Cw20CoinVerified {
            address: Addr::unchecked("addr"),
            amount: 9u64.into(),
        })
    );
}
