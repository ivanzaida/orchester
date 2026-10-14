//! The parts of Add project's form around the GitHub block: where the page is and what it does (`Page head`
//! `k3uvE`), the project's name (`Row · Project name` `P4Gkf3`, its error `RVdgw` in `gh5wV`), its folder with
//! *Choose folder…* (`Row · Project folder` `vYRWA`, `P8MAb`) and what the project keeps (`Ownership` `J9X1S`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{display, project_setup::ProjectDraft};

use super::{Actions, Form};
use crate::{
  navigation::Navigation,
  routes::Route,
  services::path_picker::PathKind,
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, ProjectsText},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::Icon,
    message::message_text,
  },
};

/// Longest folder path shown in full; a longer one keeps its last folders.
const PATH_CHARS: usize = 72;

/// *All projects › Set up Kontur*, the title and what the page does.
pub(super) fn head(ctx: &mut Ctx, form: &Form) -> Column {
  let navigation = Navigation::of(ctx);
  let here = match form.shown_name() {
    Some(name) => ctx.t_args("setup.crumbs.named", [("name", name.as_str())]),
    None => ctx.t("setup.crumbs.newProject"),
  };
  let crumbs = vec![
    Crumb::link(ctx.t("setup.crumbs.allProjects"), move || {
      navigation.go(Route::AllProjects)
    }),
    Crumb::here(here),
  ];
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(breadcrumb(ctx, "add-project-crumbs", CrumbSeparator::Chevron, crumbs))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(
          Text::new(&ctx.t("setup.title"))
            .id("add-project-title")
            .variant(TypographyStyle::Heading),
        )
        .child(Text::new(&ctx.t("setup.description")).variant(ExtraTypography::Prose.role())),
    )
}

/// The name, and once Create was pressed or something was typed, why the rules reject it.
pub(super) fn name(ctx: &mut Ctx, form: &Form) -> Column {
  let entered = form.name.get();
  let errors = ProjectDraft {
    name: &entered,
    repository: None,
  }
  .validate()
  .err()
  .and_then(|errors| errors.name);
  let shown = errors.filter(|_| form.pressed.get() || !entered.trim().is_empty());
  let label = ctx.t("setup.name");
  let mut input = FieldInput::new("project-name", label.clone(), form.name.clone());
  if let Some(error) = shown {
    input = input.error(message_text(ctx, &error));
  }
  Column::new()
    .id("project-name-row")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(Text::new(&label).variant(ExtraTypography::Strong.role()))
    .child(input.render(ctx))
}

/// The folder, *Choose folder…* — the page's action until a folder is chosen —, and what the folder is for.
pub(super) fn folder(ctx: &mut Ctx, form: &Form, actions: &Actions) -> Column {
  let folder = form.folder.get().0;
  let help = if folder.is_some() {
    ctx.t("setup.folderHelpChosen")
  } else {
    ctx.t("setup.folderHelp")
  };
  let value = match &folder {
    Some(folder) => Text::new(&display::shorten_path(folder, PATH_CHARS))
      .id("project-folder-value")
      .variant(ProjectsText::Path.role()),
    None => Text::new(&ctx.t("setup.noFolder"))
      .id("project-folder-value")
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted),
  };
  let field = Row::new()
    .id("project-folder-box")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      value
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0)),
    );
  let kind = if folder.is_some() {
    ButtonKind::Secondary
  } else {
    ButtonKind::Primary
  };
  let choose = ActionButton::new("choose-folder", kind, ctx.t("setup.chooseFolder"))
    .icon(Icon::FolderOpen)
    .in_form();
  let choose = if actions.folder.state().get().is_pending() {
    choose.loading()
  } else {
    let (pick, title) = (actions.folder.clone(), ctx.t("setup.chooseFolderTitle"));
    choose.on_click(move || pick.run((PathKind::Folder, title.clone())))
  };
  Column::new()
    .id("project-folder-row")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(Text::new(&ctx.t("setup.folder")).variant(ExtraTypography::Strong.role()))
        .child(
          Text::new(&help)
            .id("project-folder-help")
            .variant(ExtraTypography::Prose.role()),
        ),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(field)
        .child(choose),
    )
}

/// What the project keeps (`Ownership` `J9X1S`).
pub(super) fn ownership(ctx: &Ctx, form: &Form) -> Element {
  let detail = match form.shown_name() {
    Some(name) => ctx.t_args("setup.ownership.detailNamed", [("name", name.as_str())]),
    None => ctx.t("setup.ownership.detail"),
  };
  Column::new()
    .id("add-project-ownership")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Lg))
    .spacing(SpacingSize::Xs)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&ctx.t("setup.ownership.title")).variant(ProjectsText::Heading.role()))
    .child(Text::new(&detail).variant(TypographyStyle::Description))
    .into()
}
