//! What a Permissions row says (board `iPHGE`): the label carries the qualifier (*Running with*,
//! *Last ran with*, *Next run uses*; in Activity *Running with* or *Ran with*) and the value stays
//! bare (*Builder · Edit files · 3 commands · Network off · MCP asks*). A value wraps only at its
//! "·" separators, never inside a part.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::{Alignment, text_style::TextAlign},
  node::{Element, dimension::Dimension},
};
use orchester_core::permission::{AgentPermissions, CommandPermission};

use super::super::heading::agent_of;
use crate::{
  agents::permissions::{Length, parts},
  tasks::{
    detail::{RunPermissions, StepView},
    permissions::InvocationPolicy,
  },
  theme::ExtraTypography,
};

/// The row's height (`C5Ai5o`).
pub(super) const ROW_HEIGHT: f32 = 20.0;

/// Where a policy is described: the task page's row and popover, or the Activity panel about one
/// invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Wording {
  Row,
  Activity,
}

/// A row's label and value. `full` when the value lists the settings, which may need more than a
/// line; otherwise it is one short word (*starting…*, *not recorded*).
pub(super) struct PolicyText {
  pub(super) label: Arc<str>,
  pub(super) value: Arc<str>,
  pub(super) full: bool,
}

/// What `policy` says in `wording`; before any launch the step's settings for its next invocation
/// (`next`) are what it will run with.
pub(super) fn policy_text(
  ctx: &Ctx,
  step: &StepView,
  (policy, next): (&InvocationPolicy, &RunPermissions),
  wording: Wording,
) -> PolicyText {
  let ended = match wording {
    Wording::Row => "runs.permissions.label.lastRanWith",
    Wording::Activity => "runs.permissions.label.ranWith",
  };
  let running = "runs.permissions.label.runningWith";
  let word = |label: &str, value: &str| PolicyText {
    label: ctx.t(label),
    value: ctx.t(value),
    full: false,
  };
  let settings = |label: &str, permissions: &AgentPermissions, network_stays_on: bool| PolicyText {
    label: ctx.t(label),
    value: policy_value(ctx, step, permissions, network_stays_on),
    full: true,
  };
  match policy {
    // A launch is recorded as what the CLI enforced, so it never reads *Network stays on*.
    InvocationPolicy::Active(policy) => settings(running, policy, false),
    InvocationPolicy::Last(policy) => settings(ended, policy, false),
    InvocationPolicy::Starting => word(running, "runs.permissions.starting"),
    InvocationPolicy::Unknown => word(ended, "runs.permissions.notRecorded"),
    InvocationPolicy::NotStarted => word("runs.permissions.label.ranWith", "runs.permissions.notStarted"),
    InvocationPolicy::NoInvocation => settings(
      "runs.permissions.label.nextRunUses",
      &next.permissions,
      next.network_stays_on,
    ),
  }
}

/// "Builder · Edit files · 3 commands · Network off · MCP asks", breaking only at its separators.
fn policy_value(ctx: &Ctx, step: &StepView, permissions: &AgentPermissions, network_stays_on: bool) -> Arc<str> {
  let (repository, commands, network) = parts(ctx, permissions, network_stays_on, Length::Compact);
  let value = ctx.t_args(
    "runs.permissions.policyValue",
    [
      ("agent", agent_of(step)),
      ("repository", &*repository),
      ("commands", &*commands_word(ctx, permissions).unwrap_or(commands)),
      ("network", &*network),
      ("mcp", &*mcp_word(ctx, permissions)),
    ],
  );
  breaking_at_separators(&value)
}

/// *Commands ask* in ask-before-commands mode; the shared compact word otherwise.
fn commands_word(ctx: &Ctx, permissions: &AgentPermissions) -> Option<Arc<str>> {
  (*permissions.commands() == CommandPermission::AskBefore).then(|| ctx.t("runs.permissions.commandsAsk"))
}

fn mcp_word(ctx: &Ctx, permissions: &AgentPermissions) -> Arc<str> {
  ctx.t(if permissions.interactive_tool_approvals() {
    "runs.permissions.mcpAsks"
  } else {
    "runs.permissions.mcpAuto"
  })
}

/// `text` with its spaces made non-breaking except the one after each "·", so a line can only end
/// at a separator ("3 commands ·" / "Network off"), never inside a part.
pub(super) fn breaking_at_separators(text: &str) -> Arc<str> {
  text.replace(' ', "\u{a0}").replace("\u{a0}·\u{a0}", "\u{a0}· ").into()
}

/// What changed between the permissions an invocation launched with (`from`) and what the next
/// one will (`to`), in compact words: "pnpm build added", "Network on, MCP asks"; `None` when
/// nothing differs.
pub(super) fn change_words(ctx: &Ctx, from: &AgentPermissions, to: &AgentPermissions) -> Option<Arc<str>> {
  let to_parts = parts(ctx, to, false, Length::Compact);
  let mut words: Vec<Arc<str>> = Vec::new();
  if from.repository() != to.repository() || from.granted_folders() != to.granted_folders() {
    words.push(to_parts.0.clone());
  }
  match (from.commands(), to.commands()) {
    (CommandPermission::Allowed(before), CommandPermission::Allowed(after)) => {
      let added = after.entries().iter().filter(|entry| !before.entries().contains(entry));
      let removed = before.entries().iter().filter(|entry| !after.entries().contains(entry));
      words.extend(added.map(|entry| ctx.t_args("runs.permissions.changed.added", [("command", entry.as_str())])));
      words.extend(removed.map(|entry| ctx.t_args("runs.permissions.changed.removed", [("command", entry.as_str())])));
    }
    (before, after) if before != after => words.push(commands_word(ctx, to).unwrap_or(to_parts.1.clone())),
    _ => {}
  }
  if from.network() != to.network() {
    words.push(to_parts.2);
  }
  if from.interactive_tool_approvals() != to.interactive_tool_approvals() {
    words.push(mcp_word(ctx, to));
  }
  (!words.is_empty()).then(|| words.join(&ctx.t("common.listSeparator")).into())
}

/// The value in the row's text style; it wraps, right-aligned, at its separators.
pub(super) fn value_text(value: &str) -> Text {
  Text::new(value)
    .variant(ExtraTypography::CellLabel.role())
    .color(PaletteColor::TextPrimary)
    .text_align(TextAlign::Right)
}

/// The row's label, which keeps its width. It shares the value's line box (11, 1.5; muted by
/// default), as do the link and its separator, so all three sit on one line.
pub(super) fn label_text(label: &str) -> Text {
  Text::new(label)
    .variant(ExtraTypography::CellLabel.role())
    .nowrap()
    .flex_shrink(0.0)
}

/// A label and value without a link (`UPDBO`, `zCqp2`; Activity's *Ran with*), the value
/// wrapping at its separators.
pub(super) fn policy_property(text: &PolicyText) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(ROW_HEIGHT))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(label_text(&text.label))
    .child(value_text(&text.value).flex(1.0))
}

/// What `step`'s current or last invocation launched with, as a row without a link: the working
/// context once no agent can continue (`d9SVut` in 02.S2); `None` for a step without permissions
/// (a tool step).
pub fn permissions_property(ctx: &Ctx, step: &StepView) -> Option<Element> {
  let permissions = step.permissions.as_ref()?;
  let text = policy_text(ctx, step, (&permissions.invocation_policy, permissions), Wording::Row);
  Some(policy_property(&text).id("task-permissions").into())
}
