# CronCats contract for DAO queries

The contract performs these queries:

| Query                          | Description                                                                    |
| ------------------------------ | ------------------------------------------------------------------------------ |
| ProposalStatusMatches          | Get proposal status and compare it to the given status                         |
| HasPassedProposals             | Check if DAO has passed proposals, get the list of them                        |
| HasPassedProposalWithMigration | Check if DAO has passed proposals with migration message, get the list of them |
| HasProposalsGtId               | Check if the last proposal id is greater than the given value                  |

*** 

This contract doesn't support `Execute` actions and it doesn't have any state.
