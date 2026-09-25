//! Project folder field (`Y1vmV` in 01.03, `wrfvm` in 01.02): the chosen folder, *Choose
//! folder…* and a help line. The folder is chosen in the native folder dialog.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::TypographyStyle,
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use crate::{
  activity::{Activity, operations},
  background, display,
  messages::Message,
  screens::github::actions::ChosenPath,
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    field::{ValueTone, field_error, marked_value_box},
    icon::Icon,
  },
};

/// Longest folder path shown in full; longer paths keep their last folders.
const PATH_CHARS: usize = 64;
/// Height of *Choose folder…* beside the field (`Y18lT`).
const CHOOSE_HEIGHT: f32 = 40.0;

/// The dialog title and the folder chosen so far, kept when the dialog is cancelled.
pub type PickArgs = (Arc<str>, Option<ChosenPath>);
pub type PickAction = FutureAction<PickArgs, Option<ChosenPath>, Message>;

pub fn pick_action(ctx: &mut Ctx) -> PickAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(title, current): PickArgs| {
    let activity = activity.clone();
    async move {
      // Listed while the dialog is open; closing it without a folder leaves no event.
      let running = activity.start(operations::repository_pick());
      let picked = background::run(move || rfd::FileDialog::new().set_title(&*title).pick_folder()).await?;
      if picked.is_some() {
        running.finish();
      }
      Ok(picked.map(ChosenPath).or(current))
    }
  })
}

/// The field, with the folder's creation error under it. *Choose folder…* is the primary
/// action until a folder is chosen (`B1Mqb2`).
pub fn folder_field(ctx: &Ctx, pick: &PickAction, folder: Option<&PathBuf>, error: Option<Arc<str>>) -> Element {
  let picking = pick.state().get_untracked().is_pending();
  let (path, tone) = match folder {
    Some(folder) => {
      let tone = if error.is_some() {
        ValueTone::Invalid
      } else {
        ValueTone::Value
      };
      (display::shorten_path(folder, PATH_CHARS), tone)
    }
    None => (ctx.t("setup.project.noRepository").to_string(), ValueTone::Placeholder),
  };
  let mut choose = ActionButton::new(ctx.t("setup.project.chooseFolder"))
    .icon(Icon::FolderOpen)
    .id("choose-folder")
    .in_form()
    .height(Dimension::Px(CHOOSE_HEIGHT));
  if folder.is_none() {
    choose = choose.primary();
  }
  let choose = if picking {
    choose.on_click(|| {}).enabled(false)
  } else {
    let (pick, title, current) = (
      pick.clone(),
      ctx.t("setup.project.chooseRepositoryTitle"),
      folder.cloned().map(ChosenPath),
    );
    choose.on_click(move || pick.run((title.clone(), current.clone())))
  };
  let help = if folder.is_some() {
    ctx.t("setup.project.folderHelpChosen")
  } else {
    ctx.t("setup.project.folderHelp")
  };
  let column = Column::new()
    .width(Dimension::Pct(100.0))
    // Choose folder ends at the column's right edge; its focus ring reaches past it.
    .overflow_visible()
    .spacing(ExtraSpacing::Field.role())
    .child(Text::new(&ctx.t("setup.project.repository")).variant(TypographyStyle::Caption))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        // Choose folder is as tall as the row; its focus ring reaches past it.
        .overflow_visible()
        .spacing(ExtraSpacing::Stack.role())
        .align_items(Alignment::Center)
        .child(Column::new().flex(1.0).child(marked_value_box(
          ctx,
          "repository-path",
          &path,
          (tone, Icon::Folder),
          ctx.theme().form().input.height,
        )))
        .child(choose.render(ctx)),
    )
    .child(Text::new(&help).variant(TypographyStyle::Caption));
  match error {
    Some(error) => column.child(field_error(&error)).into(),
    None => column.into(),
  }
}
