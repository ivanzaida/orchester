//! Editor frame and inspector geometry from gHEgt, Oriq0 and K3w4J.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::inspector::section_title,
};

pub const INSPECTOR_WIDTH: f32 = 340.0;
const HEADER_HEIGHT: f32 = 52.0;
const INSPECTOR_HEADER: f32 = 44.0;

pub fn page() -> Column {
  Column::new()
    .id("screen-pipelines")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
}
pub fn header() -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADER_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Tabs.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
}
pub fn panel(
  ctx: &mut Ctx,
  eyebrow: &str,
  title: &str,
  body: Element,
  footer: Option<Element>,
  close: Option<super::state::State>,
) -> Element {
  panel_with_title(
    ctx,
    eyebrow,
    Text::new(title).variant(TypographyStyle::Title).into(),
    body,
    footer,
    close,
  )
}
pub fn panel_with_title(
  ctx: &mut Ctx,
  eyebrow: &str,
  title: Element,
  body: Element,
  footer: Option<Element>,
  close: Option<super::state::State>,
) -> Element {
  let close = close.map(|state| {
    crate::ui::icon_button::IconButton::new(
      "pipeline-inspector-close",
      crate::ui::icon::Icon::X,
      ctx.t("pipelines.closeInspector"),
    )
    .small()
    .on_click(move || state.update(|s| s.panel = super::state::Panel::Pipeline))
    .render(ctx)
  });
  Column::new()
    .id("pipeline-inspector")
    .width(Dimension::Px(INSPECTOR_WIDTH))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .height(Dimension::Px(INSPECTOR_HEADER))
        .width(Dimension::Pct(100.0))
        .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(Text::new(eyebrow).variant(ExtraTypography::ColumnHeading.role()))
        .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(title))
        .with_children(close),
    )
    .child(ScrollVertical::new(body).flex(1.0).min_height(Dimension::Px(0.0)))
    .with_children(footer)
    .into()
}
pub fn panel_body() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Xs)
}
pub fn section(ctx: &Ctx, key: &'static str) -> Element {
  section_title(key, &ctx.t(key), None)
    .height(Dimension::Px(30.0))
    .padding(Padding::new().top(ExtraSpacing::Loose.role()))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .into()
}
pub fn caption(text: &str) -> Text {
  Text::new(text).variant(TypographyStyle::Caption)
}
pub fn note(text: &str) -> Text {
  Text::new(text)
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
}

/// XoQDX: the change's own symbol, aligned to its first line.
pub fn change_row(ctx: &Ctx, change: &orchester_core::resource::PipelineChange) -> Row {
  use orchester_core::resource::{PipelineChange, StepChange};

  use crate::ui::icon::{Icon, icon};
  let glyph = match change {
    PipelineChange::Description => Icon::FilePen,
    PipelineChange::Step { change, .. } => match change {
      StepChange::Added => Icon::Plus,
      StepChange::Removed => Icon::Minus,
      StepChange::FailureRoute | StepChange::MaxRetries { .. } => Icon::Undo2,
      StepChange::Model | StepChange::Executor => Icon::Bot,
      StepChange::ApprovalGate { .. } => Icon::Hand,
      StepChange::Permissions | StepChange::RepositoryAccess => Icon::Shield,
      _ => Icon::FilePen,
    },
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(glyph, crate::theme::IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&crate::ui::message::message_text(
        ctx,
        &orchester_workspace::pipelines::describe::change(change),
      ))
      .variant(ExtraTypography::Prose.role())
      .flex(1.0)
      .min_width(Dimension::Px(0.0)),
    )
}
pub fn pipeline_name_error(ctx: &Ctx, error: &orchester_workspace::messages::Message) -> std::sync::Arc<str> {
  if *error == orchester_workspace::messages::name_error(&orchester_core::name::NameError::Blank) {
    ctx.t("pipelines.invalidName")
  } else {
    crate::ui::message::message_text(ctx, error)
  }
}
