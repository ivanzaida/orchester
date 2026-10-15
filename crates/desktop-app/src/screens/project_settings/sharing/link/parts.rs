//! The parts of a local project's Sharing around the link: its task list, where projects are made, and what linking
//! sends.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::sharing::ServerEntry;

use super::Props;
use crate::{
  screens::project_settings::page::{self, CONTROL_WIDTH},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    form_section::{Label, Place, section, side_row},
    icon::Icon,
    inspector::{self, PropertyValue},
  },
};

/// The local task list (`Task list` in `bWD9q`): only on this computer, with its key and tasks.
pub(super) fn task_list(ctx: &Ctx, props: &Props) -> Element {
  let name = props.project.open.project.name().as_str();
  let key = props.project.open.project.key().as_str();
  let tasks = ctx.t_args(
    "sharing.link.localTasks",
    [("key", key.to_owned()), ("count", props.facts.tasks.to_string())],
  );
  let state = Column::new()
    .width(Dimension::Px(CONTROL_WIDTH))
    .spacing(ExtraSpacing::Hair.role())
    .child(page::icon_note(
      "sharing-local",
      Icon::HardDrive,
      &ctx.t("sharing.link.localOnly"),
    ))
    .child(page::note("sharing-local-tasks", &tasks));
  let label = Label::new("sharing-task-list", ctx.t("sharing.link.taskList"))
    .description(ctx.t_args("sharing.link.taskListDescription", [("project", name), ("key", key)]));
  section(
    "project-settings-sharing-task-list",
    &ctx.t("sharing.link.taskList"),
    None,
    vec![side_row(label, state, Place::Last, true)],
  )
  .into()
}

/// *No project for this yet?* Projects are made in the server admin.
pub(super) fn admin_note(ctx: &Ctx, entry: Option<&ServerEntry>) -> Element {
  let words = entry.map_or_else(
    || ctx.t("sharing.link.noProjectYetAny").to_string(),
    |entry| {
      ctx
        .t_args(
          "sharing.link.noProjectYet",
          [
            ("server", entry.stored.name.clone()),
            ("host", entry.address.authority()),
          ],
        )
        .to_string()
    },
  );
  page::icon_note("sharing-no-project-yet", Icon::ExternalLink, &words)
}

/// *What linking sends* (`What linking sends` in `bWD9q`).
pub(super) fn sends(ctx: &Ctx) -> Element {
  Column::new()
    .id("sharing-sends")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(Text::new(&ctx.t("sharing.link.sends.title")).variant(ExtraTypography::Strong.role()))
    .child(inspector::property(
      "sharing-sends-shared",
      &ctx.t("sharing.link.sends.shared"),
      &ctx.t("sharing.link.sends.sharedValue"),
      PropertyValue::Plain,
    ))
    .child(inspector::property(
      "sharing-sends-stays",
      &ctx.t("sharing.link.sends.stays"),
      &ctx.t("sharing.link.sends.staysValue"),
      PropertyValue::Plain,
    ))
    .child(page::icon_note(
      "sharing-sends-never",
      Icon::Shield,
      &ctx.t("sharing.link.sends.never"),
    ))
    .into()
}
