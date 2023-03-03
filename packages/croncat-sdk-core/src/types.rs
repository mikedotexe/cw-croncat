use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, StdResult, Uint128};
use cw20::Cw20CoinVerified;

use crate::error::SdkError;

/// from_index: Start at the 0 index for retrieving data, unless specified for pagination
pub const DEFAULT_PAGINATION_FROM_INDEX: u64 = 0;
/// limit: will grab a total set of records or the maximum allowed.
pub const DEFAULT_PAGINATION_LIMIT: u64 = 100;

use self::gas_price_defaults::{
    GAS_ADJUSTMENT_NUMERATOR_DEFAULT, GAS_DENOMINATOR, GAS_NUMERATOR_DEFAULT,
};

pub mod gas_price_defaults {
    pub const GAS_NUMERATOR_DEFAULT: u64 = 4;
    pub const GAS_ADJUSTMENT_NUMERATOR_DEFAULT: u64 = 150;
    pub const GAS_DENOMINATOR: u64 = 100;
}

/// We can't store gas_price as floats inside cosmwasm
/// so instead of having 0.04 we use GasPrice {4/100}
/// and after that multiply Gas by `gas_adjustment` {150/100} (1.5)
#[cw_serde]
pub struct GasPrice {
    pub numerator: u64,
    /// Denominator is shared
    pub denominator: u64,
    pub gas_adjustment_numerator: u64,
}

impl GasPrice {
    pub fn is_valid(&self) -> bool {
        self.denominator != 0 && self.numerator != 0 && self.gas_adjustment_numerator != 0
    }

    pub fn calculate(&self, gas_amount: u64) -> Result<u128, SdkError> {
        let gas_adjusted = gas_amount
            .checked_mul(self.gas_adjustment_numerator)
            .and_then(|g| g.checked_div(self.denominator))
            .ok_or(SdkError::InvalidGas {})?;

        let price = gas_adjusted
            .checked_mul(self.numerator)
            .and_then(|g| g.checked_div(self.denominator))
            .ok_or(SdkError::InvalidGas {})?;

        Ok(price as u128)
    }
}

impl Default for GasPrice {
    fn default() -> Self {
        Self {
            numerator: GAS_NUMERATOR_DEFAULT,
            denominator: GAS_DENOMINATOR,
            gas_adjustment_numerator: GAS_ADJUSTMENT_NUMERATOR_DEFAULT,
        }
    }
}

#[cw_serde]
pub struct AmountForOneTask {
    // Attached balances, used for forwarding during actions
    pub cw20: Option<Cw20CoinVerified>,
    pub coin: [Option<Coin>; 2],

    // to stabilize deposited fees against point-in-time configured fees
    pub gas: u64,
    pub agent_fee: u16,
    pub treasury_fee: u16,
    pub gas_price: GasPrice,
}

impl AmountForOneTask {
    pub fn add_gas(&mut self, gas: u64) {
        self.gas = self.gas.saturating_add(gas);
    }

    pub fn add_coin(&mut self, coin: Coin) -> StdResult<bool> {
        match &mut self.coin {
            [None, None] => {
                self.coin[0] = Some(coin);
                Ok(true)
            }
            [Some(c1), None] => {
                if c1.denom == coin.denom {
                    c1.amount = c1.amount.checked_add(coin.amount)?;
                } else {
                    self.coin[1] = Some(coin);
                }
                Ok(true)
            }
            [Some(c1), Some(c2)] => {
                if c1.denom == coin.denom {
                    c1.amount = c1.amount.checked_add(coin.amount)?;

                    Ok(true)
                } else if c2.denom == coin.denom {
                    c2.amount = c2.amount.checked_add(coin.amount)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            [None, Some(_)] => unreachable!(),
        }
    }

    #[must_use]
    pub fn add_cw20(&mut self, cw20: Cw20CoinVerified) -> bool {
        if let Some(cw20_inner) = &mut self.cw20 {
            if cw20_inner.address != cw20.address {
                return false;
            }
            cw20_inner.amount += cw20.amount;
        } else {
            self.cw20 = Some(cw20);
        }
        true
    }

    pub fn sub_coin(&mut self, coin: &Coin) -> StdResult<()> {
        match &mut self.coin {
            [Some(c1), Some(c2)] => {
                if c1.denom == coin.denom {
                    c1.amount = c1.amount.checked_sub(coin.amount)?;
                } else if c2.denom == coin.denom {
                    c2.amount = c2.amount.checked_sub(coin.amount)?;
                } else {
                    Uint128::zero().checked_sub(coin.amount)?;
                }
            }
            [Some(c1), None] => {
                if c1.denom == coin.denom {
                    c1.amount = c1.amount.checked_sub(coin.amount)?;
                } else {
                    Uint128::zero().checked_sub(coin.amount)?;
                }
            }
            [None, None] => {
                Uint128::zero().checked_sub(coin.amount)?;
            }
            [None, Some(_)] => unreachable!(),
        }
        Ok(())
    }

    pub fn sub_cw20(&mut self, cw20: &Cw20CoinVerified) -> StdResult<()> {
        match &mut self.cw20 {
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
