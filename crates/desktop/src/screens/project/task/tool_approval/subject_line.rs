//! What a request is and why it asks, at the top of its details and of the Always allow step
//! (`Cv2MG` in `kpuQj`, `NV6xJ`, `XKja4`): the category chip of a command (or *Credential read*), or the
//! MCP server of a tool call; the asking profile's setting and version; and since when it waits. In
//! the Always allow step of a command, why it asks follows (`sc59y`), and the request's command box is
//! labelled with it (`J6guZs`). The reason comes from the classifier ([`CommandReason`]).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::permission::category::{CommandCategory, CommandReason};

use super::super::subject_key;
use crate::{
  display,
  tasks::approvals::{AskingSetting, ShownApproval},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{category_chip::category_chip, icon::Icon},
};

/// The chip, the policy and *waiting since*, on one line.
pub(super) fn line(ctx: &Ctx, shown: &ShownApproval) -> Element {
  let approval = &shown.approval;
  let chip = match shown.choices.category {
    Some(category) if approval.command => category_chip(
      "tool-approval-category",
      glyph(category, shown.choices.secrets),
      &ctx.t(subject_key(category, shown.choices.secrets)),
    ),
    _ => category_chip("tool-approval-category", Icon::Plug, &approval.server),
  };
  let waiting = ctx.t_args(
    "runs.toolApproval.waitingSince",
    [("time", display::clock(approval.requested_at).as_str())],
  );
  Row::new()
    .id("tool-approval-subject")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(chip)
    .with_children(policy(ctx, shown).map(|policy| {
      Text::new(&policy)
        .id("tool-approval-policy")
        .variant(TypographyStyle::Caption)
        .nowrap()
    }))
    .child(Row::new().flex(1.0))
    .child(
      Text::new(&waiting)
        .id("tool-approval-waiting")
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// The category's icon (`Qy2iD`), or a key for a credential read.
const fn glyph(category: CommandCategory, secrets: bool) -> Icon {
  if secrets {
    return Icon::KeyRound;
  }
  match category {
    CommandCategory::ReadOnly => Icon::Eye,
    CommandCategory::Worktree => Icon::FolderPen,
    CommandCategory::Network => Icon::Globe,
    CommandCategory::OutsideWrite => Icon::FolderOutput,
    CommandCategory::Unknown => Icon::CircleHelp,
    CommandCategory::HistoryDestructive => Icon::History,
  }
}

/// The asking profile's setting at the run's version (*Asks before commands · Builder v3*).
fn policy(ctx: &Ctx, shown: &ShownApproval) -> Option<Arc<str>> {
  let asking = shown.asking.as_ref()?;
  let key = match asking.setting {
    Some(AskingSetting::CommandsAsk) => "runs.toolApproval.policy.commandsAsk",
    Some(AskingSetting::NotListed) => "runs.toolApproval.policy.notListed",
    Some(AskingSetting::BeyondSandbox) => "runs.toolApproval.policy.beyondSandbox",
    Some(AskingSetting::McpActions) => "runs.toolApproval.policy.mcpActions",
    None => "runs.toolApproval.policy.profile",
  };
  let version = asking.version.to_string();
  Some(ctx.t_args(key, [("profile", &*asking.profile), ("version", version.as_str())]))
}

/// Why a command asks, as one line under the chip in the Always allow step (`sc59y`): the words, then
/// what it does it to in mono.
pub(super) fn reason(ctx: &Ctx, reason: &CommandReason) -> Element {
  let words = Words::of(ctx, reason);
  Row::new()
    .id("tool-approval-reason")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t(words.line))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .with_children(
      words
        .value
        .map(|value| Text::new(&value).variant(ExtraTypography::MonoField.role()).flex(1.0)),
    )
    .into()
}

/// The label of the request's command box with why it asks (*Command · installs a package from the
/// network: crates.io · serde*); `None` for an Unknown command, whose box says how many lines it has.
pub(super) fn command_label(ctx: &Ctx, reason: &CommandReason) -> Option<Arc<str>> {
  let words = Words::of(ctx, reason);
  Some(ctx.t_args(words.label?, [("value", &*words.value?)]))
}

/// A reason's words: the line's key, the command box label's key, and what it does it to.
struct Words {
  line: &'static str,
  label: Option<&'static str>,
  value: Option<Arc<str>>,
}

impl Words {
  fn of(ctx: &Ctx, reason: &CommandReason) -> Self {
    let (line, label, value): (_, _, &str) = match reason {
      CommandReason::Unknown => {
        return Self {
          line: "runs.toolApproval.reason.unknown",
          label: None,
          value: None,
        };
      }
      CommandReason::Installs { registry, package } => {
        let value = match package {
          Some(package) => ctx.t_args(
            "runs.toolApproval.reason.installsSource",
            [("registry", registry.as_str()), ("package", package.as_str())],
          ),
          None => registry.as_str().into(),
        };
        return Self {
          line: "runs.toolApproval.reason.installs",
          label: Some("runs.toolApproval.commandBecause.installs"),
          value: Some(value),
        };
      }
      CommandReason::DeletesOutside { path } => (
        "runs.toolApproval.reason.deletesOutside",
        "runs.toolApproval.commandBecause.deletesOutside",
        path,
      ),
      CommandReason::RewritesHistory { command } => (
        "runs.toolApproval.reason.rewritesHistory",
        "runs.toolApproval.commandBecause.rewritesHistory",
        command,
      ),
      CommandReason::ReadsCredentials { what } => (
        "runs.toolApproval.reason.readsCredentials",
        "runs.toolApproval.commandBecause.readsCredentials",
        what,
      ),
      CommandReason::WritesOutside { path } => (
        "runs.toolApproval.reason.writesOutside",
        "runs.toolApproval.commandBecause.writesOutside",
        path,
      ),
      CommandReason::Network { command } => (
        "runs.toolApproval.reason.network",
        "runs.toolApproval.commandBecause.network",
        command,
      ),
      CommandReason::Worktree { command } => (
        "runs.toolApproval.reason.worktree",
        "runs.toolApproval.commandBecause.worktree",
        command,
      ),
      CommandReason::ReadOnly { command } => (
        "runs.toolApproval.reason.readOnly",
        "runs.toolApproval.commandBecause.readOnly",
        command,
      ),
    };
    Self {
      line,
      label: Some(label),
      value: Some(value.into()),
    }
  }
}
