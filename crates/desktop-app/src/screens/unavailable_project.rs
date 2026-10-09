//! A project screen whose project cannot be opened says why, in place of the screen: no project in the
//! list has the id, its folder is gone, or its data cannot be read. The design has no board for these
//! yet; they use the placeholder's layout. Going to All projects and back opens the project again, after
//! its folder is back.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::Text,
  node::Element,
};

use super::placeholder::centered;
use crate::{
  project::Unavailable,
  ui::{icon::Icon, message::message_text},
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
  let path = unavailable.repo_root().map(|root| {
    Text::new(&root.display().to_string())
      .variant(TypographyStyle::Mono)
      .color(PaletteColor::TextMuted)
      .selectable(true)
  });
  centered(glyph, &title, &body).with_children(path).id(id).into()
}
