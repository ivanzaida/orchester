//! Per-run data (`Per-run data` `gtE4Z`): *None*, *A fresh database container* or *Files in the run's own folder* as
//! radios, the folder in a 240 field beside its radio (`ZmSi8`). A kind chosen, and the folder left, saves; the folder
//! stays as typed while another kind is chosen.

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::agent_testing::editor::form::DataKind;

use super::{
  super::super::kit::Group,
  Commit, View,
  fields::{field, input, with_error},
  readiness::beside,
  sections::{DataChoice, Sections},
};
use crate::{
  theme::ExtraSpacing,
  ui::toggle::{Toggle, ToggleKind},
};

/// The run folder's field (`Input` `Nlj7L`).
const FOLDER_WIDTH: f32 = 240.0;

/// The Per-run data group.
pub(super) fn run_data_group(ctx: &mut Ctx, sections: &Sections, view: &View, commit: &Commit) -> Element {
  let chosen = sections.data.get().0;
  let radio = |kind: DataKind, (id, key): (&str, &str), ctx: &Ctx| {
    let (data, commit) = (sections.data.clone(), commit.clone());
    Element::from(
      Toggle::new(id.to_owned(), ToggleKind::Radio, chosen == kind, ctx.t(key)).on_change(move |_| {
        data.set(DataChoice(kind));
        commit();
      }),
    )
  };
  let none = radio(
    DataKind::None,
    (
      "settings-service-data-none",
      "project_settings.services.editor.data.none",
    ),
    ctx,
  );
  let container = radio(
    DataKind::Container,
    (
      "settings-service-data-container",
      "project_settings.services.editor.data.container",
    ),
    ctx,
  );
  let folder_radio = radio(
    DataKind::Folder,
    (
      "settings-service-data-folder",
      "project_settings.services.editor.data.folder",
    ),
    ctx,
  );
  let error = view.problems().and_then(|problems| problems.run_data.as_ref());
  let folder = input(
    (
      "settings-service-data-path",
      ctx.t("project_settings.services.editor.data.path"),
    ),
    &sections.data_folder,
    commit,
  )
  .width(Dimension::Px(FOLDER_WIDTH))
  .placeholder(ctx.t("project_settings.services.editor.data.pathExample"));
  let folder = with_error(ctx, folder, error).render(ctx);
  let control = Column::new()
    .id("settings-service-data")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(none)
    .child(container)
    .child(beside(folder_radio, folder));
  let row = field(
    "settings-service-data",
    ctx.t("project_settings.services.editor.data.label"),
    ctx.t("project_settings.services.editor.data.hint"),
  )
  .render(ctx, control);
  Group::new(
    "settings-service-data-group",
    ctx.t("project_settings.services.editor.data.title"),
  )
  .row(row)
  .into()
}
