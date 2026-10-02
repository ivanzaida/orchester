//! What the invocation panel adds for an agent Orchester runs itself (04.H10 `f0xJK`): that it ran on
//! this computer and which model API it reached, whether network stayed on, and its tool calls with
//! those that were denied. Its agent reads with its model only: the endpoint row names the API.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{connection::stream::CommandStatus, id::InvocationId};

use crate::{
  tasks::{
    detail::StepView,
    transcript::{Entry, EntryKind},
  },
  ui::property::{ValueStyle, property, styled_property},
};

/// Whether `step`'s agent ran on a native connection.
pub(super) fn is_native(step: &StepView) -> bool {
  step.provider.is_some_and(|provider| provider.is_native())
}

/// *Runs* on this computer, and the *Endpoint* its model requests went to.
pub(super) fn launch_rows(ctx: &Ctx, step: &StepView) -> Vec<Element> {
  let mut rows = vec![property(
    &ctx.t("runs.activity.panel.runs"),
    &ctx.t("runs.activity.panel.runsHere"),
  )];
  rows.extend(step.endpoint.as_ref().map(|endpoint| {
    styled_property(
      &ctx.t("runs.activity.panel.endpoint"),
      &endpoint.base_url,
      ValueStyle::Path,
    )
  }));
  rows
}

/// *Network* as the agent ran with it, and its *Tool calls* with how many were denied.
pub(super) fn tool_rows(ctx: &Ctx, step: &StepView, entries: &[Entry], invocation: InvocationId) -> Vec<Element> {
  let mut rows = Vec::new();
  if let Some(permissions) = &step.permissions {
    let network = if permissions.network_stays_on {
      "runs.activity.panel.networkStaysOn"
    } else {
      "runs.activity.panel.networkOn"
    };
    rows.push(property(&ctx.t("runs.activity.panel.network"), &ctx.t(network)));
  }
  let calls: Vec<&Entry> = entries
    .iter()
    .filter(|entry| entry.invocation == invocation && matches!(entry.kind, EntryKind::Command(_)))
    .collect();
  let denied = calls
    .iter()
    .filter(
      |entry| matches!(&entry.kind, EntryKind::Command(command) if command.status == Some(CommandStatus::Declined)),
    )
    .count();
  let count = calls.len().to_string();
  let value = if denied == 0 {
    count
  } else {
    ctx
      .t_args(
        "runs.activity.panel.toolCallsDenied",
        [("count", count.as_str()), ("denied", denied.to_string().as_str())],
      )
      .to_string()
  };
  rows.push(property(&ctx.t("runs.activity.panel.toolCalls"), &value));
  rows
}
