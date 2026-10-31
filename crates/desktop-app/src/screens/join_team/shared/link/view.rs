//! What *Link a folder* draws above its actions: the project it links (`Project` `uU4TU`) and the checkout option with
//! its folder row and what the check found (`Option · Use a checkout I have` `iEWas`). The clone panel under it is
//! [`super::clone`]'s.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::SharedProject;
use orchester_workspace::sharing::project::folder::FolderCheck;

use super::Chosen;
use crate::{
  screens::settings::orchester::shown_address,
  theme::{ControlHeight, ExtraSpacing, IconSize, StartupSpacing, StartupText},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, field, field_error},
    icon::{Icon, icon},
    message::message_text,
    toggle::radio_mark,
  },
};

const FOLDER: &str = "join-team-link-folder";
const CHECK: &str = "join-team-link-check";

/// The project, its address and the repository it names.
pub(super) fn project_line(ctx: &Ctx, project: &SharedProject) -> Element {
  let hint = project.remote_hint.as_ref().map(|hint| {
    Text::new(&ctx.t_args("startup.shared.link.hint", [("hint", hint.as_str())]))
      .id("join-team-link-project-hint")
      .variant(StartupText::Sub.role())
      .nowrap()
  });
  Row::new()
    .id("join-team-link-project")
    .width(Dimension::Pct(100.0))
    .spacing(StartupSpacing::Row.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::FolderGit2, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Text::new(&project.name)
        .id("join-team-link-project-name")
        .variant(StartupText::ProjectName.role())
        .nowrap(),
    )
    .child(
      Text::new(shown_address(&project.address))
        .id("join-team-link-project-address")
        .variant(StartupText::ProjectAddress.role())
        .nowrap(),
    )
    .with_children(hint)
    .into()
}

/// The option: its radio and title, then the folder row and under it what the check found. `can_clone` when the page
/// shows the clone panel the words may point to.
pub(super) fn existing_option(
  ctx: &mut Ctx,
  (folder, choose): (&Signal<String>, &FutureAction<Arc<str>, Chosen, ()>),
  project: &SharedProject,
  (check, can_clone): (Option<&FolderCheck>, bool),
) -> Element {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(StartupSpacing::Row.role())
    .align_items(Alignment::Center)
    .child(radio_mark(true))
    .child(titles(
      "join-team-link-existing",
      &ctx.t("startup.shared.link.existing.title"),
      &ctx.t("startup.shared.link.existing.sub"),
    ));
  let row = folder_row(ctx, (folder, choose), check.is_some_and(FolderCheck::refuses));
  let found = check.map(|check| found(ctx, project, check, can_clone));
  Column::new()
    .id("join-team-link-existing")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderFocus))
    .rounded(RadiusSize::Lg)
    .child(head)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().left(StartupSpacing::OptionIndent.role()))
        .spacing(StartupSpacing::Row.role())
        .child(row)
        .with_children(found),
    )
    .into()
}

/// An option's title and what it is, 2 apart.
pub(super) fn titles(id: &str, title: &str, sub: &str) -> Column {
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(StartupText::RowTitle.role()),
    )
    .child(
      Text::new(sub)
        .id(format!("{id}-sub"))
        .variant(StartupText::Sub.role())
        .width(Dimension::Pct(100.0)),
    )
}

/// The field and *Choose…*, which lines up with the field's box, the error being under the row (`d3Cx27`, `DNRz3`).
fn folder_row(
  ctx: &mut Ctx,
  (folder, choose): (&Signal<String>, &FutureAction<Arc<str>, Chosen, ()>),
  invalid: bool,
) -> Element {
  let label = ctx.t("startup.shared.link.folder");
  let input = FieldInput::new(FOLDER, label.clone(), folder.clone())
    .placeholder(ctx.t("startup.shared.link.folderPlaceholder"))
    .text_style(StartupText::FieldMono.role())
    .box_height(ControlHeight::Lg.value());
  let input = if invalid { input.invalid() } else { input };
  let input = input.render(ctx);
  let button = ActionButton::new(
    "join-team-link-choose",
    ButtonKind::Secondary,
    ctx.t("startup.shared.link.choose"),
  )
  .icon(Icon::FolderOpen)
  .large()
  .roomy()
  .in_form();
  let button = if choose.state().get().is_pending() {
    button.loading()
  } else {
    let (choose, title) = (choose.clone(), ctx.t("startup.shared.link.chooseTitle"));
    button.on_click(move || choose.run(title.clone()))
  };
  Row::new()
    .id("join-team-link-folder-row")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::End)
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(field(
      &format!("{FOLDER}-field"),
      Some(&label),
      input,
      None,
    )))
    .child(button)
    .into()
}

/// What the check found, under the folder row.
fn found(ctx: &Ctx, project: &SharedProject, check: &FolderCheck, can_clone: bool) -> Element {
  let said = || message_text(ctx, &check.message(project));
  match check {
    FolderCheck::Matches => {
      let origin = project.remote_hint.as_ref().map_or("", |hint| hint.as_str());
      line(
        CHECK,
        (Icon::CircleCheck, PaletteColor::Success),
        &ctx.t_args("startup.shared.link.matches", [("origin", origin)]),
      )
    }
    FolderCheck::Differs { origin, hint } => Callout::new(
      "join-team-link-differs",
      CalloutTone::Warning,
      ctx.t_args("startup.shared.link.differs.title", [("origin", origin.as_str())]),
    )
    .icon(Icon::GitCompare)
    .body(ctx.t_args(
      "startup.shared.link.differs.body",
      [("project", project.name.as_str()), ("hint", hint.as_str())],
    ))
    .into(),
    FolderCheck::NotRepository => {
      let words = if can_clone {
        "startup.shared.link.notRepository"
      } else {
        "startup.shared.link.notRepositoryNoClone"
      };
      field_error("join-team-link-error", &ctx.t(words)).into()
    }
    FolderCheck::NotAbsolute
    | FolderCheck::NotLocal
    | FolderCheck::Unreadable
    | FolderCheck::LinkedElsewhere { .. } => field_error("join-team-link-error", &said()).into(),
    FolderCheck::NoHint | FolderCheck::NoOrigin | FolderCheck::LinkedHere => {
      line(CHECK, (Icon::Info, PaletteColor::TextMuted), &said())
    }
  }
}

/// A 13 mark and what it says, 6 apart (`Check` `N3vRO2`).
pub(super) fn line(id: &str, (glyph, color): (Icon, PaletteColor), text: &str) -> Element {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color).flex_shrink(0.0))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(StartupText::CheckLine.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
