use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::resource::{Tool, ToolAction};
use orchester_workspace::{agents::data::VersionState, tools::data::ToolEntry};

use crate::ui::icon::Icon;

pub(super) fn kind(tool: &Tool) -> (&'static str, Icon) {
  match tool.action() {
    ToolAction::Command(_) => ("tools.command", Icon::Terminal),
    ToolAction::Http(_) => ("tools.http", Icon::Globe),
    ToolAction::DatabaseQuery(_) => ("tools.database", Icon::Database),
  }
}
/// How many pipeline steps call a tool: *Unused*, *1 pipeline step*, *3 pipeline steps*.
pub(super) fn used(ctx: &Ctx, count: usize) -> Arc<str> {
  match count {
    0 => ctx.t("tools.unused"),
    1 => ctx.t("tools.usedStepOne"),
    _ => ctx.t_args("tools.usedSteps", [("count", count.to_string())]),
  }
}
pub(super) fn version(ctx: &Ctx, entry: &ToolEntry) -> Arc<str> {
  match entry.state {
    VersionState::Published(v) => ctx.t_args("tools.published", [("version", v.get().to_string())]),
    VersionState::Draft { published: Some(v) } => ctx.t_args(
      "tools.draftPublished",
      [
        ("draft", v.get().saturating_add(1).to_string()),
        ("version", v.get().to_string()),
      ],
    ),
    VersionState::Draft { published: None } => ctx.t("tools.draftOnly"),
    VersionState::StaleDraft { published, .. } => ctx.t_args(
      "tools.stale",
      [("version", published.map_or_else(|| "0".into(), |v| v.get().to_string()))],
    ),
  }
}
pub(super) fn editor_state(
  ctx: &Ctx,
  entry: Option<&ToolEntry>,
  unchanged: bool,
  busy: bool,
) -> (Arc<str>, Icon, PaletteColor) {
  if busy {
    return (ctx.t("tools.saving"), Icon::LoaderCircle, PaletteColor::TextSecondary);
  }
  let Some(entry) = entry else {
    return (ctx.t("tools.notSaved"), Icon::Pencil, PaletteColor::TextMuted);
  };
  if !unchanged {
    let text = entry.state.published().map_or(ctx.t("tools.unsaved"), |v| {
      ctx.t_args("tools.unsavedPublished", [("version", v.get().to_string())])
    });
    return (text, Icon::Pencil, PaletteColor::TextSecondary);
  }
  match entry.state {
    VersionState::Published(_) => (version(ctx, entry), Icon::CircleCheck, PaletteColor::Success),
    VersionState::StaleDraft { .. } => (version(ctx, entry), Icon::GitBranch, PaletteColor::Warning),
    VersionState::Draft { published: Some(v) } => (
      ctx.t_args("tools.draftSavedPublished", [("version", v.get().to_string())]),
      Icon::Check,
      PaletteColor::TextSecondary,
    ),
    VersionState::Draft { published: None } => (version(ctx, entry), Icon::Pencil, PaletteColor::TextMuted),
  }
}
pub(super) fn inspector_version(ctx: &Ctx, entry: &ToolEntry) -> Arc<str> {
  let key = if entry.state.has_draft() {
    "tools.compactDraftVersion"
  } else {
    "tools.compactVersion"
  };
  let number = if entry.state.has_draft() {
    entry.state.published().map_or(1, |v| v.get().saturating_add(1))
  } else {
    entry.state.published().map_or(1, |v| v.get())
  };
  ctx.t_args(key, [("version", number.to_string())])
}
