//! Data correlation for the forthcoming native accepted-read adapter.
//! This does not establish current authority, claim a dispatch, or read a page.
use crate::ledger::DispatchPermit;
use avesra_contracts::{
    ErrorCode,
    browser::reading::{Reply, Request},
};

pub fn validate_dispatch(request: &Request, permit: &DispatchPermit) -> Result<(), ErrorCode> {
    request.matches_action(&permit.action)?;
    let context = &request.context;
    if context.dispatch.uuid() != permit.dispatch_id
        || context.source.device.uuid() != permit.device_id
        || context.source.session.uuid() != permit.session_id
        || context.source.capture_epoch != permit.capture_epoch
        || context.source.action_epoch != permit.action_epoch
    {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}

/// Only bounded untrusted evidence is validated here. Actual owner must still
/// recheck its original deadline, cancellation, live browser/scope/document and
/// Store::validate_dispatch before publication. No opaque authority is minted.
pub fn validate_reply(
    request: &Request,
    reply: &Reply,
    permit: &DispatchPermit,
) -> Result<(), ErrorCode> {
    validate_dispatch(request, permit)?;
    reply.validate(request)
}
