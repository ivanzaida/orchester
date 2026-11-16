//! What a local MCP server's panel says besides its fields: the chips under its head, the note while it has edits that
//! are not saved (`Edit bar` `Z4FS4` in L8), the refusal of a save (`Notice · danger` `lGnr1` in L12) and, at the
//! bottom, why it is not ready (`Note · warning` `o5aBJo` in L2).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::sharing::Missing;
use orchester_workspace::{
  messages::Message,
  readiness::{Group, ReadinessStanding},
};

use super::super::{model::Entry, words};
use crate::ui::{
  chip::BadgeTone,
  icon::Icon,
  message::message_text,
  notice::{Notice, NoticeTone},
};

/// The first part the server lacks.
pub(super) fn first_missing(entry: &Entry) -> Option<&Missing> {
  match &entry.standing {
    Some(ReadinessStanding::Missing(missing)) => missing.first(),
    _ => None,
  }
}

/// *Unsaved changes* while it has edits, then how it stands: *Needs the program path*, *Needs a secret value*, *Ready*
/// or *Off*.
pub(super) fn chips(ctx: &Ctx, entry: &Entry, changed: bool) -> Vec<(Arc<str>, BadgeTone)> {
  let mut chips = Vec::new();
  if changed {
    chips.push((ctx.t("local_hub.chip.unsaved"), BadgeTone::Warning));
  }
  chips.push(match (entry.group, first_missing(entry)) {
    (Group::OffHere, _) => (ctx.t("local_hub.chip.off"), BadgeTone::Neutral),
    (Group::NeedsYouHere, Some(Missing::LocalMcpProgram { .. })) => {
      (ctx.t("local_hub.chip.needsProgram"), BadgeTone::Warning)
    }
    (Group::NeedsYouHere, Some(Missing::LocalSecret { .. })) => {
      (ctx.t("local_hub.chip.needsSecret"), BadgeTone::Warning)
    }
    (Group::NeedsYouHere, _) => (
      entry.standing.as_ref().map_or_else(
        || ctx.t("local_hub.chip.ready"),
        |standing| words::needs(ctx, entry, standing),
      ),
      BadgeTone::Warning,
    ),
    (Group::Ready, _) => (ctx.t("local_hub.chip.ready"), BadgeTone::Success),
  });
  chips
}

/// *used by Designer, Reviewer* at the chips' end; nothing when no profile uses the server.
pub(super) fn used_by(ctx: &Ctx, entry: &Entry) -> Option<Arc<str>> {
  (!entry.used_by.is_empty()).then(|| ctx.t_args("local_hub.panel.usedBy", [("names", entry.used_by.join(", "))]))
}

/// *Editing figma · not saved yet* over what changed and what *Save* and *Cancel* do.
pub(super) fn editing(ctx: &Ctx, name: &str, changed: &[Arc<str>]) -> Element {
  let fields = changed.join(", ");
  Notice::new(
    "local-hub-mcp-unsaved",
    NoticeTone::Neutral,
    ctx.t_args("local_hub.mcp.editingTitle", [("name", name)]),
  )
  .icon(Icon::Pencil)
  .title_style(crate::theme::HubText::NoticeTitle.role())
  .body(ctx.t_args("local_hub.mcp.editingBody", [("fields", fields)]))
  .flat()
  .into()
}

/// *Couldn't save figma: …* over what stays.
pub(super) fn refusal(ctx: &Ctx, name: &str, (reason, secret): (&Message, bool)) -> Element {
  let body = if secret {
    "local_hub.mcp.refusedSecretBody"
  } else {
    "local_hub.mcp.refusedBody"
  };
  Notice::new(
    "local-hub-mcp-refused",
    NoticeTone::Danger,
    ctx.t_args(
      "local_hub.mcp.refusedTitle",
      [
        ("name", name.to_owned()),
        ("reason", message_text(ctx, reason).to_string()),
      ],
    ),
  )
  .icon(Icon::CircleX)
  .title_style(crate::theme::HubText::NoticeTitle.role())
  .body(ctx.t(body))
  .flat()
  .into()
}

/// *Not ready: set the program path. Runs whose pipeline uses figma don't start until then.*
pub(super) fn not_ready(ctx: &Ctx, entry: &Entry) -> Option<Element> {
  if entry.group != Group::NeedsYouHere {
    return None;
  }
  let (key, args): (&str, Vec<(&str, String)>) = match first_missing(entry)? {
    Missing::LocalMcpProgram { .. } => ("local_hub.mcp.notReadyProgram", vec![("name", entry.name.clone())]),
    Missing::LocalSecret { name, .. } => (
      "local_hub.mcp.notReadySecret",
      vec![("name", entry.name.clone()), ("secret", name.to_string())],
    ),
    _ => return None,
  };
  Some(
    Notice::new("local-hub-mcp-not-ready", NoticeTone::Warning, ctx.t_args(key, args))
      .icon(Icon::CircleAlert)
      .title_style(crate::theme::HubText::Value.role())
      .flat()
      .into(),
  )
}
