//! The parts of a local project's Sharing around the link: its task list, where projects are made, and what linking
//! sends.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::sharing::ServerEntry;

use super::Props;
use crate::{
  screens::project_settings::page::{self, CONTROL_WIDTH},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchesterText},
  ui::{
    form_section::{Label, Place, section, side_row},
    icon::{Icon, icon},
  },
};

/// The label column of *What linking sends* (`Shared with members` in `Fy5KW`).
const SENDS_LABEL: f32 = 180.0;

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
      Icon::Monitor,
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
pub fn sends(ctx: &Ctx) -> Element {
  card(
    ctx,
    "sharing-sends",
    ("sharing.link.sends.title", "sharing.link.sends.sharedValue"),
  )
}

/// *What joining sends* (`What joining sends` in `AiCwO`): the ticked tasks only.
pub fn join_sends(ctx: &Ctx) -> Element {
  card(
    ctx,
    "sharing-join-sends",
    ("sharing.join.sends.title", "sharing.join.sends.sharedValue"),
  )
}

/// The card `id`: its title and what is shared with members, by their keys, what stays here and what never goes.
fn card(ctx: &Ctx, id: &str, (title, shared): (&str, &str)) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Inset.role())
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(Text::new(&ctx.t(title)).variant(OrchesterText::CardTitle.role()))
    .child(sends_row(
      &format!("{id}-shared"),
      &ctx.t("sharing.link.sends.shared"),
      &ctx.t(shared),
    ))
    .child(sends_row(
      &format!("{id}-stays"),
      &ctx.t("sharing.link.sends.stays"),
      &ctx.t("sharing.link.sends.staysValue"),
    ))
    .child(
      Row::new()
        .id(format!("{id}-never"))
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::ShieldCheck, IconSize::Sm, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t("sharing.link.sends.never"))
            .variant(ExtraTypography::Note.role())
            .flex(1.0),
        ),
    )
    .into()
}

/// A line of the card: what, in a 180 column of `text_muted`, and the things, in `text_primary` (`Shared` `othgy`).
fn sends_row(id: &str, label: &str, value: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(
      Text::new(label)
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(SENDS_LABEL))
        .flex_shrink(0.0),
    )
    .child(
      Text::new(value)
        .id(format!("{id}-value"))
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextPrimary)
        .flex(1.0),
    )
}
