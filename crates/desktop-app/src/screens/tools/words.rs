use std::sync::Arc;

use lurq::app::ctx::Ctx;
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
