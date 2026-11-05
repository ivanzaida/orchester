//! The parts of the Services tab around its rows: the stack line (`Summary` `T92RJ`: a 13 `text_secondary` `layers`,
//! the title in 13/600, its facts in 12 `text_muted`, 10 apart, *Stop all* at its end), the column heads (`Columns`
//! `H3c4q`: 11.5 `text_muted`, 14 at the sides and between, as wide as the rows' cells), and the note under the rows
//! (`Note` `p0Oloj`: a 13 `text_muted` `info` and 12 `text_muted`, 8 apart).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::service::Service;
use orchester_ipc::services::RunService;
use orchester_workspace::tasks::detail::StepView;

use super::{
  row::{ACTIONS_WIDTH, NAME_WIDTH, PORT_WIDTH, STATE_WIDTH, UPTIME_WIDTH},
  words,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The stack line, with *Stop all* while `stop_all` is given: the button, pressed or under way.
pub(super) fn stack_line(
  ctx: &Ctx,
  (services, definitions): (&[RunService], &[Service]),
  step: Option<&StepView>,
  stop_all: Option<ActionButton>,
) -> Element {
  Row::new()
    .id("task-services-stack")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Layers, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(&words::title(ctx, services, definitions))
        .id("task-services-stack-title")
        .variant(TaskText::SectionTitle.role())
        .nowrap(),
    )
    .child(
      Text::new(&words::detail(ctx, services, step))
        .id("task-services-stack-detail")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(stop_all)
    .into()
}

/// *Stop all*, which stops every service that runs.
pub(super) fn stop_all(ctx: &Ctx) -> ActionButton {
  ActionButton::new(
    "task-services-stop-all",
    ButtonKind::Ghost,
    ctx.t("run_services.header.stopAll"),
  )
  .icon(Icon::Square)
}

/// The column heads.
pub(super) fn columns(ctx: &Ctx) -> Element {
  let head = |id: &str, key: &str, width: Option<f32>| {
    let text = Text::new(&ctx.t(key))
      .id(format!("task-services-column-{id}"))
      .variant(ExtraTypography::Note.role())
      .nowrap();
    match width {
      Some(width) => Row::new().width(Dimension::Px(width)).flex_shrink(0.0).child(text),
      None => Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(text),
    }
  };
  Row::new()
    .id("task-services-columns")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(ExtraSpacing::Inset.role())
        .right(ExtraSpacing::Inset.role()),
    )
    .spacing(ExtraSpacing::Inset.role())
    .child(head("state", "run_services.columns.state", Some(STATE_WIDTH)))
    .child(head("service", "run_services.columns.service", Some(NAME_WIDTH)))
    .child(head("port", "run_services.columns.port", Some(PORT_WIDTH)))
    .child(head("address", "run_services.columns.address", None))
    .child(head("uptime", "run_services.columns.uptime", Some(UPTIME_WIDTH)))
    .child(Row::new().width(Dimension::Px(ACTIONS_WIDTH)).flex_shrink(0.0))
    .into()
}

/// The note under the rows: when Orchester stops the services, the step named when it is known.
pub(super) fn note(ctx: &Ctx, step: Option<&StepView>) -> Element {
  let words: Arc<str> = match step {
    Some(step) => ctx.t_args(
      "run_services.note",
      [("step", words::step_label(ctx, step).to_string())],
    ),
    None => ctx.t("run_services.noteNoStep"),
  };
  Row::new()
    .id("task-services-note")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .id("task-services-note-text")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
