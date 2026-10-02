//! What the invocation panel adds for an agent Orchester runs itself (04.H10 `f0xJK`): that it ran on
//! this computer, the endpoint its connection names now (an invocation records none), whether network
//! stayed on, and its tool calls with those that were denied. Its agent reads with its model only.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::Text,
  node::Element,
};
use orchester_core::{
  connection::stream::CommandStatus, id::InvocationId, run::Invocation, time::Timestamp, tokens::compact_tokens,
};

use super::super::super::ProjectNav;
use crate::{
  agents::permissions::{Length, parts},
  display,
  navigation::route::{AgentSelection, Page},
  tasks::{
    detail::{RunView, StepView},
    transcript::{Entry, EntryKind},
  },
  ui::{
    message_text,
    pressable::surface,
    property::{ValueStyle, element_property, property, styled_property},
  },
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
  // The invocation records no endpoint: the row says it is the connection's, as it is now.
  rows.extend(
    step
      .endpoint
      .as_ref()
      .map(|endpoint| styled_property(&ctx.t("runs.activity.panel.endpointNow"), endpoint, ValueStyle::Path)),
  );
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

/// "Implement invocation" (`FRtSj`).
pub(super) fn title(ctx: &Ctx, step: &StepView) -> Arc<str> {
  ctx.t_args("runs.activity.panel.nativeTitle", [("step", &*step.name)])
}

/// *Started* with how long the invocation has run, or ran: "14:25 · 7 min" (`eK8lM`).
pub(super) fn started_row(ctx: &Ctx, invocation: &Invocation) -> Element {
  let end = invocation.ended_at.unwrap_or_else(Timestamp::now);
  let took = {
    // A clock set back after the start would make it negative: it reads as no time at all.
    let millis = (end.unix_millis() - invocation.started_at.unix_millis()).max(0);
    Duration::from_millis(millis.unsigned_abs())
  };
  let value = ctx.t_args(
    "runs.activity.panel.startedValue",
    [
      ("start", display::clock(invocation.started_at).as_str()),
      ("took", &*message_text(ctx, &display::duration(took))),
    ],
  );
  property(&ctx.t("runs.activity.panel.started"), &value)
}

/// *Run budget*: the run's counted tokens of its budget, "248k / 1.4M tokens" (`IbPlN`).
pub(super) fn budget_row(ctx: &Ctx, run: &RunView) -> Element {
  let value = ctx.t_args(
    "tokens.usedOfBudget",
    [
      ("used", compact_tokens(run.usage().counted()).as_str()),
      ("budget", compact_tokens(run.run.budget().get()).as_str()),
    ],
  );
  property(&ctx.t("runs.activity.panel.runBudget"), &value)
}

/// *Permissions* in compact words without the agent and network, which have rows of their own, as a
/// link to the profile: "Edit files · 3 commands ↗" (`EZD41`).
pub(super) fn permissions_row(ctx: &Ctx, nav: &ProjectNav, step: &StepView) -> Option<Element> {
  let permissions = step.permissions.as_ref()?;
  let (repository, commands, _) = parts(
    ctx,
    &permissions.permissions,
    permissions.network_stays_on,
    Length::Compact,
  );
  let value = ctx.t_args(
    "runs.activity.panel.permissionsValue",
    [("repository", &*repository), ("commands", &*commands)],
  );
  let link = nav
    .link(Page::Agents(AgentSelection::Profile(permissions.profile_id)))
    .on(surface().id("activity-invocation-permissions"))
    .child(
      Text::new(&value)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary),
    );
  Some(element_property(&ctx.t("runs.summary.permissions"), link.into()))
}
