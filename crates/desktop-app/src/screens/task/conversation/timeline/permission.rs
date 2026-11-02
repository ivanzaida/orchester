//! What the run's permissions menu changed (`Event lines` `BaB32` in 15f `CimC4`): a quiet row 36 in, a 13 glyph, what
//! changed in 12 `text_secondary`, then who and when in 12 `text_muted`, 10 apart. *Approving everything in this run*
//! (`shield-alert` in `warning`) ends in *Stop* while it lasts; *Stopped approving everything* (`shield-check`); a
//! revoked rule (`x`) names what it answered in Geist Mono 12; automation's refused widening (`lock`) ends in
//! *Review…*, which opens the chip's menu. The actions in 12/500 `text_secondary`.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  run::{Answerer, PermissionEventKind, RuleScope, RunPermissionChange},
  time::Timestamp,
};
use orchester_workspace::{display, tasks::timeline::permission_events::PermissionLine};

use super::{Timeline, entry::subject_key};
use crate::{
  screens::task::conversation::permissions::Permissions,
  theme::{ConversationSpacing, ConversationText, ExtraSpacing, ExtraTypography, IconSize, PermissionsText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

pub(super) fn row(ctx: &mut Ctx, timeline: &Timeline, (key, line, at): (&str, &PermissionLine, Timestamp)) -> Element {
  let id = format!("conversation-permission-{key}");
  let time = display::clock(at);
  let time = time.as_str();
  let (glyph, color, lead) = match &line.kind {
    PermissionEventKind::Everything { .. } => (
      Icon::ShieldAlert,
      PaletteColor::Warning,
      "conversation.permissions.event.everything",
    ),
    PermissionEventKind::Stopped { .. } => (
      Icon::ShieldCheck,
      PaletteColor::TextMuted,
      "conversation.permissions.event.stopped",
    ),
    PermissionEventKind::Revoked { .. } => (
      Icon::X,
      PaletteColor::TextMuted,
      "conversation.permissions.event.revoked",
    ),
    PermissionEventKind::Refused => (
      Icon::Lock,
      PaletteColor::TextMuted,
      "conversation.permissions.event.refused",
    ),
  };
  let detail = ctx.t_args(detail_key(line), [("time", time)]);
  let command = match &line.kind {
    PermissionEventKind::Revoked { scope } => Some(
      Text::new(&rule_words(scope))
        .id(format!("{id}-command"))
        .variant(ExtraTypography::PropertyMono.role())
        .nowrap(),
    ),
    _ => None,
  };
  let lead = ctx.t(lead);
  let action = action(ctx, timeline, &id, line);
  Row::new()
    .id(id.clone())
    .describe("kind", kind_name(&line.kind))
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(ConversationSpacing::ApprovalRow.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color))
    .child(
      Text::new(&lead)
        .id(format!("{id}-text"))
        .variant(ConversationText::ApprovalLead.role())
        .nowrap(),
    )
    .with_children(command)
    .child(
      Text::new(&detail)
        .id(format!("{id}-detail"))
        .variant(ConversationText::ApprovalDetail.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(action)
    .into()
}

/// *Stop* on the approving everything that lasts while the run goes on; *Review…* on a refusal while it goes on.
fn action(ctx: &mut Ctx, timeline: &Timeline, id: &str, line: &PermissionLine) -> Option<Element> {
  let run = timeline
    .detail
    .run
    .as_ref()
    .filter(|run| !run.run.status().is_terminal())?;
  let permissions = Permissions::of(ctx);
  let (key, press): (&str, Arc<dyn Fn() + Send + Sync>) = match &line.kind {
    PermissionEventKind::Everything { .. } if line.lasting => {
      let (root, run) = (timeline.chat_project.root.clone(), run.id());
      (
        "conversation.permissions.event.stop",
        Arc::new(move || permissions.ask(&root, run, RunPermissionChange::StopApprovingEverything)),
      )
    }
    PermissionEventKind::Refused => {
      let open = permissions.open;
      (
        "conversation.permissions.event.review",
        Arc::new(move || open.set(true)),
      )
    }
    _ => return None,
  };
  Some(
    pressable(move || press())
      .id(format!("{id}-action"))
      .describe("state", "enabled")
      .child(Text::new(&ctx.t(key)).variant(PermissionsText::Action.role()).nowrap())
      .into(),
  )
}

fn detail_key(line: &PermissionLine) -> &'static str {
  let person = line.by == Answerer::Person;
  match &line.kind {
    PermissionEventKind::Everything { was } if was.is_autopilot() => {
      "conversation.permissions.event.detail.everythingWasAutopilot"
    }
    PermissionEventKind::Everything { .. } => "conversation.permissions.event.detail.everythingWasManual",
    PermissionEventKind::Stopped { back_to } => match (person, back_to.is_autopilot()) {
      (true, true) => "conversation.permissions.event.detail.stoppedPersonAutopilot",
      (true, false) => "conversation.permissions.event.detail.stoppedPersonManual",
      (false, true) => "conversation.permissions.event.detail.stoppedAutomationAutopilot",
      (false, false) => "conversation.permissions.event.detail.stoppedAutomationManual",
    },
    PermissionEventKind::Revoked { .. } if person => "conversation.permissions.event.detail.revokedPerson",
    PermissionEventKind::Revoked { .. } => "conversation.permissions.event.detail.revokedAutomation",
    PermissionEventKind::Refused => "conversation.permissions.event.detail.refused",
  }
}

/// The name `describe` reports a line by, for tests and QA.
const fn kind_name(kind: &PermissionEventKind) -> &'static str {
  match kind {
    PermissionEventKind::Everything { .. } => "everything",
    PermissionEventKind::Stopped { .. } => "stopped",
    PermissionEventKind::Revoked { .. } => "revoked",
    PermissionEventKind::Refused => "refused",
  }
}

/// What a rule answers, as written: a command's program and subcommand, an MCP server and its tool, a prefix.
fn rule_words(scope: &RuleScope) -> Arc<str> {
  match scope {
    RuleScope::SimilarCommand { command } => command.words().into(),
    RuleScope::SimilarMcpTool { server, tool, .. } => format!("{server} {tool}").into(),
    RuleScope::AlwaysCommand { prefix } => prefix.as_str().into(),
  }
}

/// What a rule answers, and what of it and since when, as the permissions menu lists it.
pub(in crate::screens::task::conversation) fn rule_text(
  ctx: &Ctx,
  scope: &RuleScope,
  since: Timestamp,
) -> (Arc<str>, Arc<str>) {
  let time = display::clock(since);
  let time = time.as_str();
  let detail = match scope {
    RuleScope::SimilarCommand { command } => {
      let category = ctx.t(subject_key(command.category(), false));
      ctx.t_args(
        "conversation.permissions.similar.command",
        [("category", &*category), ("time", time)],
      )
    }
    RuleScope::SimilarMcpTool {
      folder: Some(folder), ..
    } => ctx.t_args(
      "conversation.permissions.similar.folder",
      [("folder", folder.as_str()), ("time", time)],
    ),
    RuleScope::SimilarMcpTool { check: Some(check), .. } => ctx.t_args(
      "conversation.permissions.similar.check",
      [("check", check.as_str()), ("time", time)],
    ),
    RuleScope::SimilarMcpTool { .. } | RuleScope::AlwaysCommand { .. } => {
      ctx.t_args("conversation.permissions.similar.tool", [("time", time)])
    }
  };
  (rule_words(scope), detail)
}
