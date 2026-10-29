//! Permissions › Approvals (07 `J1ixkr`, 07b `VOpn8`, `docs/design/approval-modes.md`): Autopilot and Manual as radio
//! rows with one line each; under Manual its allowed commands and *Ask before MCP actions that require approval*, under
//! Autopilot what still asks the person; *Approve automatically* while a profile from before approval modes has it on;
//! a question about a widening of the group as its last row, and the decisions note under the box. Turning Autopilot
//! on widens and is asked about in place; switching to Manual never asks.
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{
    ApprovalMode,
    capability::{PermissionOption, support},
  },
};

use super::{CommandList, Context, commands, section};
use crate::{
  screens::{agents::fields::Fields, hub::parts::note},
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText},
  ui::{
    chip::TagChip,
    icon::Icon,
    toggle::{Toggle, ToggleKind},
  },
};

/// What Autopilot never approves, in the order 07b lists it.
const STILL_ASKS: [(&str, &str); 3] = [
  ("outside", "agents.permissions.approvals.stillAsks.outside"),
  ("push", "agents.permissions.approvals.stillAsks.push"),
  ("refs", "agents.permissions.approvals.stillAsks.refs"),
];

/// The group, with `asked` as the last row inside its box.
pub(super) fn group(
  ctx: &mut Ctx,
  context: &Context<'_>,
  provider: Option<AgentProvider>,
  asked: Option<Element>,
) -> Element {
  let fields = context.fields;
  let mode = fields.permissions.get().0.approval_mode();
  let mut rows = vec![mode_row(ctx, fields, ApprovalMode::Autopilot, mode)];
  if mode == ApprovalMode::Autopilot {
    rows.push(still_asks(ctx));
  }
  rows.push(mode_row(ctx, fields, ApprovalMode::Manual, mode));
  if mode == ApprovalMode::Manual {
    rows.push(allowed_commands(ctx, context, provider));
    // MCP servers, and so their approvals, are a CLI's (a native connection offers none).
    if provider.is_some_and(|p| !p.is_native()) {
      rows.push(ask_tools(ctx, fields));
    }
  }
  // Runs still approve everything automatically for a profile set so before approval modes (or over MCP): it shows
  // while it is on, so the person sees it and can turn it off.
  if fields.permissions.get().0.auto_approve() {
    rows.push(auto_approve(ctx, fields));
  }
  rows.extend(asked);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(section(
      "agent-approvals",
      (&ctx.t("agents.permissions.approvals.title"), None),
      rows,
    ))
    .child(note(
      "agent-approvals-decisions",
      Icon::Info,
      &ctx.t("agents.permissions.approvals.decisions"),
    ))
    .into()
}

/// The name of `mode` as the group says it, also in the draft bar's *Manual → Autopilot*.
pub(in crate::screens::agents) fn mode_name(ctx: &Ctx, mode: ApprovalMode) -> std::sync::Arc<str> {
  ctx.t(match mode {
    ApprovalMode::Autopilot => "agents.permissions.approvals.autopilot.title",
    ApprovalMode::Manual => "agents.permissions.approvals.manual.title",
  })
}

fn mode_row(ctx: &Ctx, fields: &Fields, mode: ApprovalMode, chosen: ApprovalMode) -> Element {
  let (id, hint) = match mode {
    ApprovalMode::Autopilot => ("agent-mode-autopilot", "agents.permissions.approvals.autopilot.hint"),
    ApprovalMode::Manual => ("agent-mode-manual", "agents.permissions.approvals.manual.hint"),
  };
  let title = mode_name(ctx, mode);
  let value = fields.permissions.clone();
  let toggle = Toggle::new(id, ToggleKind::Radio, mode == chosen, title.clone())
    .hide_label()
    .on_change(move |_| value.update(|p| p.0.set_approval_mode(mode)));
  Row::new()
    .id(format!("{id}-row"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(toggle)
    .child(text(&title, &ctx.t(hint)))
    .into()
}

/// A block under a radio row, indented past the radio.
fn nested(id: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(HubSpacing::Nested.role())
        .right(HubSpacing::Row.role())
        .top(HubSpacing::FieldY.role())
        .bottom(HubSpacing::FieldY.role()),
    )
    .spacing(HubSpacing::FieldGap.role())
    .align_items(Alignment::Center)
}

/// A label with its hint under it, filling the row.
fn text(label: &str, hint: &str) -> Column {
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(Text::new(label).variant(ExtraTypography::Strong.role()))
    .child(
      Text::new(hint)
        .variant(HubText::Hint.role())
        .width(Dimension::Pct(100.0)),
    )
}

/// Manual's *Allowed commands*: the list, or why the connection cannot enforce one; a profile set to any command in
/// the worktree before approval modes says so.
fn allowed_commands(ctx: &mut Ctx, context: &Context<'_>, provider: Option<AgentProvider>) -> Element {
  let fields = context.fields;
  let option = PermissionOption::AllowedCommands;
  let enforced = provider.is_some_and(|p| support(p, Platform::current(), option).is_enforced());
  let unavailable = (!enforced).then(|| super::super::enforcement::unavailable(ctx, provider, option));
  let any = matches!(
    fields.permissions.get().0.commands(),
    orchester_core::permission::CommandPermission::AnyInWorktree
  );
  let list = ctx.mount_keyed::<CommandList>(
    "agent-command-list",
    commands::Props {
      fields: fields.clone(),
      unavailable,
    },
  );
  let column = text(
    &ctx.t("agents.allowed"),
    &ctx.t("agents.permissions.approvals.allowedHint"),
  )
  .with_children(any.then(|| {
    Text::new(&ctx.t("agents.permissions.approvals.anyCommand"))
      .id("agent-commands-any")
      .variant(HubText::Hint.role())
      .width(Dimension::Pct(100.0))
  }))
  .child(
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(ExtraSpacing::Snug.role()))
      .child(list),
  );
  nested("agent-commands").child(column).into()
}

/// *Ask before MCP actions that require approval*, under Manual.
fn ask_tools(ctx: &Ctx, fields: &Fields) -> Element {
  let on = fields.permissions.get().0.interactive_tool_approvals();
  let title = ctx.t("agents.askTools");
  let value = fields.permissions.clone();
  let toggle = Toggle::new("agent-ask-tools", ToggleKind::Switch, on, title.clone())
    .hide_label()
    .on_change(move |on| value.update(|p| p.0.set_interactive_tool_approvals(on)));
  nested("agent-ask-tools-row")
    .child(text(&title, &ctx.t("agents.toolCallsHint")))
    .child(toggle)
    .into()
}

/// *Approve automatically (don't ask me)*, a profile's setting from before approval modes that answers every request,
/// what Autopilot still asks too; only turned off here.
fn auto_approve(ctx: &Ctx, fields: &Fields) -> Element {
  let title = ctx.t("agents.permissions.approvals.autoApprove.title");
  let value = fields.permissions.clone();
  let toggle = Toggle::new("agent-auto", ToggleKind::Switch, true, title.clone())
    .hide_label()
    .on_change(move |on| value.update(|p| p.0.set_auto_approve(on)));
  Row::new()
    .id("agent-auto-row")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(HubSpacing::FieldGap.role())
    .align_items(Alignment::Center)
    .child(text(&title, &ctx.t("agents.permissions.approvals.autoApprove.hint")))
    .child(toggle)
    .into()
}

/// Autopilot's *Still asks you*: what it never approves, as chips, and what Sources and temp folders are.
fn still_asks(ctx: &Ctx) -> Element {
  let chips = STILL_ASKS.iter().fold(
    Row::new()
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(ExtraSpacing::Snug.role()),
    |chips, (id, key)| chips.child(TagChip::new(format!("agent-still-asks-{id}"), ctx.t(key))),
  );
  let column = text(
    &ctx.t("agents.permissions.approvals.stillAsks.title"),
    &ctx.t("agents.permissions.approvals.stillAsks.hint"),
  )
  .child(
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(ExtraSpacing::Snug.role()))
      .child(chips),
  )
  .child(Text::new(&ctx.t("agents.permissions.approvals.stillAsks.note")).variant(HubText::Hint.role()));
  nested("agent-still-asks").child(column).into()
}
