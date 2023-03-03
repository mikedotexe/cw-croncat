# CronCats contract for balances queries

The contract can perform these queries:

| Query                  | Description                                                             |
| ---------------------- | ----------------------------------------------------------------------- |
| GetBalance             | Get native balance of the address                                       |
| GetCw20Balance         | Get cw20 balance of the address                                         |
| HasBalanceComparator   | Compare balance of the address to the given coin (either native or cw20)|

*** 

This contract doesn't support `Execute` actions and it doesn't have any state.
