//! A project screen whose project cannot be opened says why, in place of the screen: no project in the
//! list has the id, its folder is gone, or its data cannot be read. The design has no board for these
//! yet; they are the UI kit's empty state (`Empty project` `LeIOm`) with the project's folder under it.
//! Going to All projects and back opens the project again, once its folder is back.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};

use crate::{
  project::Unavailable,
  ui::{
    detail_row::{DetailValue, detail_row},
    icon::Icon,
    message::message_text,
    state::EmptyState,
  },
};

pub fn screen(ctx: &mut Ctx, unavailable: &Unavailable) -> Element {
  let (id, glyph, title, body) = match unavailable {
    Unavailable::Unknown => (
      "screen-project-unknown",
      Icon::CircleHelp,
      ctx.t("project.unknown.title"),
      ctx.t("project.unknown.body"),
    ),
    Unavailable::FolderMissing { name, .. } => (
      "screen-project-folder-missing",
      Icon::FolderX,
      ctx.t_args("project.folderMissing.title", [("name", name.as_str())]),
      ctx.t("project.folderMissing.body"),
    ),
    Unavailable::Unreadable { name, reason, .. } => (
      "screen-project-unreadable",
      Icon::CircleAlert,
      ctx.t_args("project.unreadable.title", [("name", name.as_str())]),
      message_text(ctx, reason),
    ),
    Unavailable::Failed(reason) => (
      "screen-project-failed",
      Icon::CircleAlert,
      ctx.t("project.failed.title"),
      message_text(ctx, reason),
    ),
  };
  let mut card = EmptyState::new(id, glyph, title, body);
  if let Some(root) = unavailable.repo_root() {
    card = card.child(detail_row(
      &format!("{id}-folder"),
      &ctx.t("project.folder"),
      &root.display().to_string(),
      DetailValue::Mono,
    ));
  }
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .child(card)
    .into()
}
