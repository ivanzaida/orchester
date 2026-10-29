//! One module of the list (`Webhooks v2` `l5w8QF` in `01 · Modules`): 40 high, 8 at its sides, its parts 12 apart,
//! radius 6, the `hover` fill under the pointer, opening the module's Summary. A 14 `target`, the name in 13/500 with
//! its description 8 after it in 11.5 `text_muted`, both cut to their room; the forecast in 12 in its tone (an open
//! module's only), the dates in 11 `text_muted` and the progress, a 56 meter and *N of M tasks done* in 12
//! `text_secondary`, at the row's end.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, module::ModuleState};
use orchester_workspace::{delivery::Forecast, modules::data::ModuleSummary};

use super::words;
use crate::{
  navigation::Navigation,
  routes::{ModuleTab, ProjectPage, Route},
  theme::{ExtraColor, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    meter::{MeterTone, meter},
    pressable::Press,
    skeleton::{self, bone},
    tooltip::Truncated,
  },
};

/// The row's height and the widths at its end (`l5w8QF`): the dates 130, the progress 180 with its 56 meter.
const ROW_HEIGHT: f32 = 40.0;
const DATES_WIDTH: f32 = 130.0;
const PROGRESS_WIDTH: f32 = 180.0;
const METER_WIDTH: f32 = 56.0;

/// `summary`'s row, its forecast once it is read; the row's element id is `modules-row-<index>`.
pub fn row(
  ctx: &mut Ctx,
  (project, index): (ProjectId, usize),
  summary: &ModuleSummary,
  forecast: Option<&Forecast>,
) -> Element {
  let id = format!("modules-row-{index}");
  let details = summary.module.details();
  let open = details.state() == ModuleState::Open;
  let forecast = forecast.filter(|_| open).map(|forecast| {
    let (words, tone) = words::forecast_row(ctx, forecast, details.target());
    Text::new(&words)
      .id(format!("{id}-forecast"))
      .variant(TypographyStyle::Description)
      .color(tone)
      .nowrap()
      .flex_shrink(0.0)
  });
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(icon(
      Icon::Target,
      IconSize::Md,
      if open {
        PaletteColor::TextSecondary
      } else {
        PaletteColor::TextMuted
      },
    ))
    .child(name(ctx, &id, summary))
    .with_children(forecast)
    .child(
      Row::new()
        .width(Dimension::Px(DATES_WIDTH))
        .flex_shrink(0.0)
        .justify(Justify::End)
        .child(
          Text::new(&words::dates(ctx, details))
            .id(format!("{id}-dates"))
            .variant(TypographyStyle::Caption)
            .nowrap(),
        ),
    )
    .child(progress(ctx, &id, summary));
  let navigation = Navigation::of(ctx);
  let page = ProjectPage::Module(summary.module.id(), ModuleTab::default());
  Press::Open(navigation.to(Route::Project(project, page)))
    .button()
    .id(id)
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("module", details.name().as_str())
    .describe("state", words::state_key(details.state()))
    .child(line)
    .into()
}

/// A module's row while the modules are read (`05 · Modules · loading` `beo7S`): its mark, name, description, forecast,
/// dates and progress as bones in the row's own geometry; `(name, description, forecast)` are their lengths.
pub fn bones(ctx: &Ctx, (name, description, forecast): (f32, f32, f32)) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(bone(IconSize::Md.size(), IconSize::Md.size()))
    .child(
      Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(skeleton::line(ctx, ExtraTypography::Strong.role(), name))
        .child(skeleton::line(ctx, ExtraTypography::Note.role(), description)),
    )
    .child(skeleton::line(ctx, TypographyStyle::Description, forecast))
    .child(
      Row::new()
        .width(Dimension::Px(DATES_WIDTH))
        .flex_shrink(0.0)
        .justify(Justify::End)
        .child(skeleton::line(ctx, TypographyStyle::Caption, DATES_WIDTH - 26.0)),
    )
    .child(
      Row::new()
        .width(Dimension::Px(PROGRESS_WIDTH))
        .flex_shrink(0.0)
        .spacing(SpacingSize::Sm)
        .justify(Justify::End)
        .align_items(Alignment::Center)
        .child(bone(METER_WIDTH, 3.0))
        .child(skeleton::line(ctx, TypographyStyle::Description, PROGRESS_WIDTH - METER_WIDTH - 16.0)),
    )
}

/// The name and the description after it, each cut to its room.
fn name(ctx: &mut Ctx, id: &str, summary: &ModuleSummary) -> Element {
  let details = summary.module.details();
  let description = (!details.description().is_empty()).then(|| {
    Row::new().flex_shrink(1.0).min_width(Dimension::Px(0.0)).child(
      Truncated::new(
        format!("{id}-description"),
        details.description().to_owned(),
        ExtraTypography::Note.role(),
      )
      .render(ctx),
    )
  });
  Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Row::new().flex_shrink(0.0).max_width(Dimension::Pct(100.0)).child(
        Truncated::new(
          format!("{id}-name"),
          details.name().as_str().to_owned(),
          ExtraTypography::Strong.role(),
        )
        .render(ctx),
      ),
    )
    .with_children(description)
    .into()
}

/// A meter of the work done and *N of M tasks done*.
fn progress(ctx: &Ctx, id: &str, summary: &ModuleSummary) -> Element {
  let (done, scope) = (u64::from(summary.progress.done), u64::from(summary.progress.scope));
  let words: Arc<str> = words::progress(ctx, summary.progress);
  Row::new()
    .id(format!("{id}-progress"))
    .width(Dimension::Px(PROGRESS_WIDTH))
    .flex_shrink(0.0)
    .spacing(SpacingSize::Sm)
    .justify(Justify::End)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .width(Dimension::Px(METER_WIDTH))
        .flex_shrink(0.0)
        .child(meter(&format!("{id}-meter"), (done, scope.max(1)), MeterTone::Plain)),
    )
    .child(
      Text::new(&words)
        .id(format!("{id}-done"))
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .into()
}
