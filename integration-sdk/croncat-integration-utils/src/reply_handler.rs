use crate::error::CronCatContractError;
use crate::CronCatTaskExecutionInfo;
use cosmwasm_std::{from_binary, Binary, Reply};
use cw_utils::parse_reply_execute_data;

/// Reply handler when a contract calls [`create_task`](croncat_sdk_tasks::msg::TasksExecuteMsg::CreateTask).
/// This will handle [`reply_always`](cosmwasm_std::ReplyOn::Always) covering success and failure.
pub fn reply_handle_croncat_task_creation(
    msg: Reply,
) -> Result<(CronCatTaskExecutionInfo, Binary), CronCatContractError> {
    let reply_id = msg.id;
    if msg.clone().result.into_result().is_err() {
        return Err(CronCatContractError::ReplyError { reply_id });
    }

    let msg_parsed = parse_reply_execute_data(msg)?;
    let msg_binary = msg_parsed
        .data
        .ok_or(CronCatContractError::ReplyError { reply_id })?;
    let created_task_info: CronCatTaskExecutionInfo = from_binary(&msg_binary)?;

    // We return the newly-created task details
    // in your contract's state if you wish.
    // Please see the create-task-handle-tick example for info:
    // https://github.com/CronCats/cw-purrbox/tree/main/contracts
    Ok((created_task_info, msg_binary))
}
