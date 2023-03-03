use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, StdResult, Uint128};
use croncat_sdk_core::types::GasPrice;
use cw20::Cw20CoinVerified;

use crate::error::SdkError;

#[cw_serde]
pub struct TaskBalanceResponse {
    pub balance: Option<TaskBalance>,
}
#[cw_serde]
pub struct TaskBalance {
    pub native_balance: Uint128,
    pub cw20_balance: Option<Cw20CoinVerified>,
    pub ibc_balance: Option<Coin>,
}

impl TaskBalance {
    pub fn verify_enough_attached(
        &self,
        native_required: Uint128,
        cw20_required: Option<Cw20CoinVerified>,
        ibc_required: Option<Coin>,
        recurring: bool,
        native_denom: &str,
    ) -> Result<(), SdkError> {
        let multiplier = if recurring {
            Uint128::new(2)
        } else {
            Uint128::new(1)
        };
        if self.native_balance < native_required * multiplier {
            return Err(SdkError::NotEnoughNative {
                denom: native_denom.to_owned(),
                lack: native_required * multiplier - self.native_balance,
            });
        }
        self.verify_enough_cw20(cw20_required, multiplier)?;
        match (ibc_required, &self.ibc_balance) {
            (Some(req), Some(attached)) => {
                if req.denom != attached.denom {
                    return Err(SdkError::NotEnoughNative {
                        denom: req.denom,
                        lack: req.amount * multiplier,
                    });
                }
                if attached.amount < req.amount * multiplier {
                    return Err(SdkError::NotEnoughNative {
                        denom: req.denom,
                        lack: req.amount * multiplier - attached.amount,
                    });
                }
            }
            (Some(req), None) => {
                return Err(SdkError::NotEnoughNative {
                    denom: req.denom,
                    lack: req.amount * multiplier,
                })
            }
            // Dont want untracked or differing CW20s from required
            (None, Some(_)) => {
                return Err(SdkError::NonRequiredDenom {});
            }
            // nothing attached, nothing required
            (None, None) => (),
        }
        Ok(())
    }

    pub fn verify_enough_cw20(
        &self,
        cw20_required: Option<Cw20CoinVerified>,
        multiplier: Uint128,
    ) -> Result<(), SdkError> {
        match (cw20_required, &self.cw20_balance) {
            (Some(req), Some(attached)) => {
                if req.address != attached.address {
                    return Err(SdkError::NotEnoughCw20 {
                        addr: req.address.into_string(),
                        lack: req.amount * multiplier,
                    });
                }
                if attached.amount < req.amount * multiplier {
                    return Err(SdkError::NotEnoughCw20 {
                        addr: req.address.into_string(),
                        lack: req.amount * multiplier - attached.amount,
                    });
                }
                Ok(())
            }
            (Some(req), None) => Err(SdkError::NotEnoughCw20 {
                addr: req.address.into_string(),
                lack: req.amount * multiplier,
            }),
            // Dont want untracked or differing CW20s from required
            (None, Some(_)) => Err(SdkError::NonRequiredDenom {}),
            // nothing attached, nothing required
            (None, None) => Ok(()),
        }
    }

    pub fn sub_coin(&mut self, coin: &Coin, native_denom: &str) -> StdResult<()> {
        if coin.denom == native_denom {
            self.native_balance = self.native_balance.checked_sub(coin.amount)?;
        } else {
            match &mut self.ibc_balance {
                Some(task_coin) if task_coin.denom == coin.denom => {
                    task_coin.amount = task_coin.amount.checked_sub(coin.amount)?;
                }
                _ => {
                    // If denoms doesn't match it means we have zero coins
                    Uint128::zero().checked_sub(coin.amount)?;
                }
            }
        }
        Ok(())
    }

    pub fn sub_cw20(&mut self, cw20: &Cw20CoinVerified) -> StdResult<()> {
        match &mut self.cw20_balance {
            Some(task_cw20) if task_cw20.address == cw20.address => {
                task_cw20.amount = task_cw20.amount.checked_sub(cw20.amount)?;
            }
            _ => {
                // If addresses doesn't match it means we have zero coins
                Uint128::zero().checked_sub(cw20.amount)?;
            }
        }
        Ok(())
    }
}

#[cw_serde]
pub struct Config {
    // Runtime
    pub owner_addr: Addr,

    /// A multisig admin whose sole responsibility is to pause the contract in event of emergency.
    /// Must be a different contract address than DAO, cannot be a regular keypair
    /// Does not have the ability to unpause, must rely on the DAO to assess the situation and act accordingly
    pub pause_admin: Addr,

    /// Address of the croncat_factory
    pub croncat_factory_addr: Addr,

    /// Key to query address of the tasks
    pub croncat_tasks_key: (String, [u8; 2]),
    /// Key to query address of the agents
    pub croncat_agents_key: (String, [u8; 2]),

    // Economics
    pub agent_fee: u16,
    pub treasury_fee: u16,
    pub gas_price: GasPrice,

    // Treasury
    pub treasury_addr: Option<Addr>,
    pub cw20_whitelist: Vec<Addr>,
    pub native_denom: String,

    // The default query limit
    pub limit: u64,
}

#[cw_serde]
pub struct UpdateConfig {
    pub agent_fee: Option<u16>,
    pub treasury_fee: Option<u16>,
    pub gas_price: Option<GasPrice>,
    pub croncat_tasks_key: Option<(String, [u8; 2])>,
    pub croncat_agents_key: Option<(String, [u8; 2])>,
    pub treasury_addr: Option<String>,
    /// Add supported cw20s
    /// That's seems unfair to undo support of cw20's after user already created a task with it
    pub cw20_whitelist: Option<Vec<String>>,
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{coin, Addr, Uint128};
    use cw20::Cw20CoinVerified;

    use crate::SdkError;

    use super::{GasPrice, TaskBalance};

    #[test]
    fn gas_price_validation() {
        assert!(!GasPrice {
            numerator: 0,
            denominator: 1,
            gas_adjustment_numerator: 1
        }
        .is_valid());

        assert!(!GasPrice {
            numerator: 1,
            denominator: 0,
            gas_adjustment_numerator: 1
        }
        .is_valid());

        assert!(!GasPrice {
            numerator: 1,
            denominator: 1,
            gas_adjustment_numerator: 0
        }
        .is_valid());

        assert!(GasPrice {
            numerator: 1,
            denominator: 1,
            gas_adjustment_numerator: 1
        }
        .is_valid());
    }

    #[test]
    fn gas_price_calculate_test() {
        // Test with default values
        let gas_price_wrapper = GasPrice::default();
        let gas_price = 0.04;
        let gas_adjustments = 1.5;

        let gas = 200_000;
        let expected = gas as f64 * gas_adjustments * gas_price;
        assert_eq!(expected as u128, gas_price_wrapper.calculate(gas).unwrap());

        let gas = 160_000;
        let expected = gas as f64 * gas_adjustments * gas_price;
        assert_eq!(expected as u128, gas_price_wrapper.calculate(gas).unwrap());

        let gas = 1_234_000;
        let expected = gas as f64 * gas_adjustments * gas_price;
        assert_eq!(expected as u128, gas_price_wrapper.calculate(gas).unwrap());

        // Check custom works
        let gas_price_wrapper = GasPrice {
            numerator: 25,
            denominator: 100,
            gas_adjustment_numerator: 120,
        };
        let gas_price = 0.25;
        let gas_adjustments = 1.2;

        let gas = 200_000;
        let expected = gas as f64 * gas_adjustments * gas_price;
        assert_eq!(expected as u128, gas_price_wrapper.calculate(gas).unwrap());

        let gas = 160_000;
        let expected = gas as f64 * gas_adjustments * gas_price;
        assert_eq!(expected as u128, gas_price_wrapper.calculate(gas).unwrap());

        let gas = 1_234_000;
        let expected = gas as f64 * gas_adjustments * gas_price;
        assert_eq!(expected as u128, gas_price_wrapper.calculate(gas).unwrap());
    }

    #[test]
    fn verify_enough_attached_ok_test() {
        let native_balance = Uint128::from(100u64);
        let cw20 = Cw20CoinVerified {
            address: Addr::unchecked("addr"),
            amount: Uint128::from(100u64),
        };
        let ibc_coin = coin(100, "ibc");

        // Test when cw20_balance and ibc_balance are None
        let task_balance = TaskBalance {
            native_balance,
            cw20_balance: None,
            ibc_balance: None,
        };
        assert!(task_balance
            .verify_enough_attached(Uint128::from(100u64), None, None, false, "denom")
            .is_ok());
        assert!(task_balance
            .verify_enough_attached(Uint128::from(50u64), None, None, true, "denom")
            .is_ok());

        // Test with cw20_balance and ibc_balance
        let task_balance = TaskBalance {
            native_balance,
            cw20_balance: Some(cw20.clone()),
            ibc_balance: Some(ibc_coin.clone()),
        };
        // TODO:
        println!(
            "HHHHHHHH = {:?}",
            task_balance.verify_enough_attached(Uint128::from(100u64), None, None, false, "denom")
        );
        // We're now validating you're not adding tokens that never get used, #noMoreBlackHoles
        assert!(task_balance
            .verify_enough_attached(Uint128::from(100u64), None, None, false, "denom")
            .is_err());
        assert!(task_balance
            .verify_enough_attached(Uint128::from(50u64), None, None, true, "denom")
            .is_err());
        assert!(task_balance
            .verify_enough_attached(
                Uint128::from(100u64),
                Some(cw20),
                Some(ibc_coin),
                false,
                "denom"
            )
            .is_ok());
        assert!(task_balance
            .verify_enough_attached(
                Uint128::from(50u64),
                Some(Cw20CoinVerified {
                    address: Addr::unchecked("addr"),
                    amount: Uint128::from(50u64),
                }),
                Some(coin(50, "ibc")),
                true,
                "denom"
            )
            .is_ok());
    }

    #[test]
    fn verify_enough_attached_err_test() {
        let native_balance = Uint128::from(100u64);
        let cw20 = Cw20CoinVerified {
            address: Addr::unchecked("addr"),
            amount: Uint128::from(100u64),
        };
        let ibc_coin = coin(100, "ibc");

        // Test when cw20_balance and ibc_balance are None, native_balance is not sufficient
        let task_balance = TaskBalance {
            native_balance,
            cw20_balance: None,
            ibc_balance: None,
        };
        assert_eq!(
            task_balance
                .verify_enough_attached(Uint128::from(101u64), None, None, false, "denom")
                .unwrap_err(),
            SdkError::NotEnoughNative {
                denom: "denom".to_owned(),
                lack: 1u64.into(),
            }
        );
        assert_eq!(
            task_balance
                .verify_enough_attached(native_balance, Some(cw20.clone()), None, false, "denom")
                .unwrap_err(),
            SdkError::NotEnoughCw20 {
                addr: "addr".to_owned(),
                lack: 100u64.into(),
            }
        );
        assert_eq!(
            task_balance
                .verify_enough_attached(
                    native_balance,
                    None,
                    Some(ibc_coin.clone()),
                    false,
                    "denom"
                )
                .unwrap_err(),
            SdkError::NotEnoughNative {
                denom: "ibc".to_owned(),
                lack: 100u64.into(),
            }
        );

        // Test when cw20_balance or ibc_balance are not sufficient
        let task_balance = TaskBalance {
            native_balance,
            cw20_balance: Some(cw20.clone()),
            ibc_balance: Some(ibc_coin.clone()),
        };
        // cw20_balance is not sufficient
        assert_eq!(
            task_balance
                .verify_enough_attached(
                    Uint128::from(100u64),
                    Some(Cw20CoinVerified {
                        address: Addr::unchecked("addr"),
                        amount: Uint128::from(101u64),
                    }),
                    Some(ibc_coin.clone()),
                    false,
                    "denom"
                )
                .unwrap_err(),
            SdkError::NotEnoughCw20 {
                addr: "addr".to_owned(),
                lack: 1u64.into(),
            }
        );
        // cw20_balance has another address
        assert_eq!(
            task_balance
                .verify_enough_attached(
                    Uint128::from(100u64),
                    Some(Cw20CoinVerified {
                        address: Addr::unchecked("addr2"),
                        amount: Uint128::from(100u64),
                    }),
                    Some(ibc_coin),
                    false,
                    "denom"
                )
                .unwrap_err(),
            SdkError::NotEnoughCw20 {
                addr: "addr2".to_owned(),
                lack: 100u64.into(),
            }
        );
        // ibc_balance is not sufficient
        assert_eq!(
            task_balance
                .verify_enough_attached(
                    Uint128::from(100u64),
                    Some(cw20.clone()),
                    Some(coin(101, "ibc")),
                    false,
                    "denom"
                )
                .unwrap_err(),
            SdkError::NotEnoughNative {
                denom: "ibc".to_owned(),
                lack: 1u64.into(),
            }
        );
        // ibc_balance has another denom
        assert_eq!(
            task_balance
                .verify_enough_attached(
                    Uint128::from(100u64),
                    Some(cw20),
                    Some(coin(100, "ibc2")),
                    false,
                    "denom"
                )
                .unwrap_err(),
            SdkError::NotEnoughNative {
                denom: "ibc2".to_owned(),
                lack: 100u64.into(),
            }
        );
    }

    #[test]
    fn sub_coin_test() {
        let native_balance = Uint128::from(100u64);
        let ibc_coin = coin(100, "ibc");

        let mut task_balance = TaskBalance {
            native_balance,
            cw20_balance: None,
            ibc_balance: Some(ibc_coin.clone()),
        };

        task_balance
            .sub_coin(&coin(10, "native"), "native")
            .unwrap();
        assert_eq!(
            task_balance,
            TaskBalance {
                native_balance: Uint128::from(90u64),
                cw20_balance: None,
                ibc_balance: Some(ibc_coin),
            }
        );

        task_balance.sub_coin(&coin(1, "ibc"), "native").unwrap();
        assert_eq!(
            task_balance,
            TaskBalance {
                native_balance: Uint128::from(90u64),
                cw20_balance: None,
                ibc_balance: Some(coin(99, "ibc")),
            }
        );

        assert!(task_balance
            .sub_coin(&coin(91, "native"), "native")
            .is_err());

        assert!(task_balance.sub_coin(&coin(100, "ibc"), "native").is_err());

        assert!(task_balance
            .sub_coin(&coin(100, "wrong"), "native")
            .is_err());
    }

    #[test]
    fn sub_cw20_test() {
        let native_balance = Uint128::from(100u64);
        let cw20 = Cw20CoinVerified {
            address: Addr::unchecked("addr"),
            amount: Uint128::from(100u64),
        };

        let mut task_balance = TaskBalance {
            native_balance,
            cw20_balance: Some(cw20),
            ibc_balance: None,
        };

        task_balance
            .sub_cw20(&Cw20CoinVerified {
                address: Addr::unchecked("addr"),
                amount: Uint128::from(10u64),
            })
            .unwrap();
        assert_eq!(
            task_balance,
            TaskBalance {
                native_balance,
                cw20_balance: Some(Cw20CoinVerified {
                    address: Addr::unchecked("addr"),
                    amount: Uint128::from(90u64),
                }),
                ibc_balance: None,
            }
        );

        assert!(task_balance
            .sub_cw20(&Cw20CoinVerified {
                address: Addr::unchecked("addr"),
                amount: Uint128::from(91u64),
            })
            .is_err());

        assert!(task_balance
            .sub_cw20(&Cw20CoinVerified {
                address: Addr::unchecked("addr2"),
                amount: Uint128::from(1u64),
            })
            .is_err());
    }
}
