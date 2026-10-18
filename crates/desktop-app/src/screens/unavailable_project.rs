//! A project screen whose project cannot be opened says why, in place of the screen: no project in the
//! list has the id, its folder is gone, or its data cannot be read. The design has no board for these
//! yet; they are the UI kit's empty state (`Empty project` `LeIOm`) with the project's folder under it,
//! *Go to All projects*, and *Try again* for a folder that may be back. Data a newer Orchester wrote
//! says how to keep working with it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;

use crate::{
  navigation::Navigation,
  project::{Projects, Unavailable},
  routes::Route,
  ui::{
    button::{ActionButton, ButtonKind},
    detail_row::{DetailValue, detail_row},
    icon::Icon,
    message::message_text,
    state::EmptyState,
  },
};

pub fn screen(ctx: &mut Ctx, project: ProjectId, unavailable: &Unavailable) -> Element {
  let (id, glyph, title, body) = words(ctx, unavailable);
  let mut card = EmptyState::new(id, glyph, title, body);
  if let Unavailable::Unreadable { newer_data: true, .. } = unavailable {
    card = card.child(Text::new(&ctx.t("project.unreadable.newerData")).variant(TypographyStyle::Description));
  }
  if let Some(root) = unavailable.repo_root() {
    card = card.child(detail_row(
      &format!("{id}-folder"),
      &ctx.t("project.folder"),
      &root.display().to_string(),
      DetailValue::Mono,
    ));
  }
  if let Unavailable::FolderMissing { .. } = unavailable {
    let projects = Projects::of(ctx);
    card = card.action(
      ActionButton::new(format!("{id}-try-again"), ButtonKind::Primary, ctx.t("common.tryAgain"))
        .icon(Icon::RotateCcw)
        .on_click(move || projects.retry(project)),
    );
  }
  let navigation = Navigation::of(ctx);
  card = card.action(
    ActionButton::new(
      format!("{id}-all-projects"),
      ButtonKind::Secondary,
      ctx.t("project.allProjects"),
    )
    .icon(Icon::Folders)
    .opens(navigation.to(Route::AllProjects)),
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .child(card)
    .into()
}

/// The element id, glyph, title and explanation of `unavailable`'s screen.
fn words(ctx: &Ctx, unavailable: &Unavailable) -> (&'static str, Icon, Arc<str>, Arc<str>) {
  match unavailable {
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
  }
}
