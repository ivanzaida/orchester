//! *Add a checkout folder…* on the landing (`d8oDsO`) and what came of it: a folder that isn't a git checkout shows as
//! a faded card that says why, with the refusal under it; the note beside the button says what to do next.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{display::shorten_path, sharing::project::landing::Landing};

use super::{Added, Said};
use crate::{
  theme::{DISABLED_OPACITY, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
  },
};

/// The most characters of a folder's path the card shows before it cuts the middle.
const PATH: usize = 48;

/// The repository the project names, or its name without a hint.
fn hint(landing: &Landing) -> String {
  landing
    .project
    .remote_hint
    .as_ref()
    .map_or_else(|| landing.project.name.clone(), |hint| hint.as_str().to_owned())
}

/// A folder that couldn't be added: a faded card for one that isn't a git checkout, and why under it.
pub(super) fn refused(ctx: &Ctx, landing: &Landing, add: &FutureAction<Arc<str>, Added, Said>) -> Vec<Element> {
  let state = add.state().get();
  if state.is_pending() {
    return Vec::new();
  }
  match state.data {
    Some(Added::NotGit(folder)) => {
      let path = folder.display().to_string();
      let text = ctx.t_args("sharing.landing.notGit", [("path", path), ("hint", hint(landing))]);
      vec![
        not_git_card(ctx, &folder),
        field_error("orchester-landing-refused", &text).into(),
      ]
    }
    Some(Added::Failed(folder)) => {
      let text = ctx.t_args("sharing.landing.notAdded", [("path", folder.display().to_string())]);
      vec![field_error("orchester-landing-refused", &text).into()]
    }
    Some(Added::Closed | Added::Project(_)) | None => Vec::new(),
  }
}

/// The folder as a card that can't be picked: its name and path, that it has no `.git`, and what that means.
fn not_git_card(ctx: &Ctx, folder: &std::path::Path) -> Element {
  let name = folder
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_default();
  Column::new()
    .id("orchester-landing-not-git")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Inset.role())
    .spacing(ExtraSpacing::Fine.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .opacity(DISABLED_OPACITY)
    .describe("state", "unavailable")
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&name).variant(ExtraTypography::Strong.role()).nowrap())
        .child(
          Text::new(&shorten_path(folder, PATH))
            .variant(OrchesterText::Address.role())
            .color(PaletteColor::TextMuted)
            .nowrap(),
        ),
    )
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleX, IconSize::Sm, PaletteColor::Danger))
        .child(
          Text::new(&ctx.t("sharing.landing.notGitLine"))
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::Danger),
        ),
    )
    .child(
      Text::new(&ctx.t("sharing.landing.notGitNote"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// *Add a checkout folder…* with what to do next: pick another folder after a refusal, pick one before going on when
/// none can be picked, or what adding one does; the page's `actions` at its end (`Actions` `JZzjA`).
pub(super) fn add_row(
  ctx: &Ctx,
  landing: &Landing,
  (add, none): (&FutureAction<Arc<str>, Added, Said>, bool),
  actions: [Element; 2],
) -> Element {
  let state = add.state().get();
  let button =
    ActionButton::new("orchester-landing-add", ButtonKind::Ghost, ctx.t("sharing.landing.add")).icon(Icon::FolderPlus);
  let button = if state.is_pending() {
    button.loading()
  } else {
    let (add, title) = (add.clone(), ctx.t("sharing.landing.addTitle"));
    button.on_click(move || add.run(title.clone()))
  };
  let pending = state.is_pending();
  let note = match state.data.filter(|_| !pending) {
    Some(Added::NotGit(_) | Added::Failed(_)) => ctx.t("sharing.landing.addAnother"),
    Some(Added::Project(_)) => ctx.t("sharing.landing.addedNote"),
    _ if none => ctx.t("sharing.landing.addFirst"),
    _ => ctx.t_args("sharing.landing.addHint", [("hint", hint(landing))]),
  };
  Row::new()
    .id("orchester-landing-add-row")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Xs))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(button)
    .child(
      Text::new(&note)
        .id("orchester-landing-add-note")
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(actions)
    .into()
}
