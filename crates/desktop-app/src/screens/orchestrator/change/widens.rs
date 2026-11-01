//! What a draft widens, in the short form of a card's *Widens* row (`UKgrR`: `network: … · new CLI program psql`):
//! each widening the publishing rules name in a few words, with what it names.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_workspace::messages::Message;

use crate::ui::message::message_text;

/// `reason` in the short form of the *Widens* row; a widening without one reads as the publishing rules word it.
pub(super) fn compact(ctx: &Ctx, reason: &Message) -> Arc<str> {
  let Some(key) = short_key(reason.key) else {
    return message_text(ctx, reason);
  };
  ctx.t_args(key, reason.args.iter().map(|(name, value)| (*name, value.as_str())))
}

/// The short form of the widening worded by `key`, for the widenings a publication can name.
fn short_key(key: &str) -> Option<&'static str> {
  Some(match key {
    "appMcp.widens.repository" => "orchestrator.change.widens.repository",
    "appMcp.widens.writableFolder" => "orchestrator.change.widens.writableFolder",
    "appMcp.widens.commands" => "orchestrator.change.widens.commands",
    "appMcp.widens.network" => "orchestrator.change.widens.network",
    "appMcp.widens.toolCallsUnasked" => "orchestrator.change.widens.toolCallsUnasked",
    "appMcp.widens.account" => "orchestrator.change.widens.account",
    "appMcp.widens.mcpServers" => "orchestrator.change.widens.mcpServers",
    "appMcp.widens.variable" => "orchestrator.change.widens.variable",
    "appMcp.widens.firstPipelineVersion" | "appMcp.projectTools.widens.firstVersion" => {
      "orchestrator.change.widens.firstVersion"
    }
    "appMcp.widens.pipelineSteps" => "orchestrator.change.widens.pipelineSteps",
    "appMcp.widens.pipelineSources" => "orchestrator.change.widens.pipelineSources",
    "appMcp.widens.pipelineStep" => "orchestrator.change.widens.pipelineStep",
    "appMcp.widens.approvalGate" => "orchestrator.change.widens.approvalGate",
    "appMcp.widens.stepToolCalls" => "orchestrator.change.widens.stepToolCalls",
    "appMcp.projectTools.widens.program" => "orchestrator.change.widens.program",
    "appMcp.projectTools.widens.arguments" => "orchestrator.change.widens.arguments",
    "appMcp.projectTools.widens.exitCodes" => "orchestrator.change.widens.exitCodes",
    _ => return None,
  })
}
