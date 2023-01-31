/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.19.0.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

export interface InstantiateMsg {}
export type ExecuteMsg = string;
export type QueryMsg = {
  proposal_status_matches: ProposalStatusMatches;
} | {
  has_passed_proposals: {
    dao_address: string;
  };
} | {
  has_passed_proposal_with_migration: {
    dao_address: string;
  };
} | {
  has_proposals_gt_id: {
    dao_address: string;
    value: number;
  };
};
export type Status = "open" | "rejected" | "passed" | "executed" | "closed" | "execution_failed";
export interface ProposalStatusMatches {
  dao_address: string;
  proposal_id: number;
  status: Status;
  [k: string]: unknown;
}
export type Binary = string;
export interface QueryResponseForBinary {
  data: Binary;
  result: boolean;
}