//! Library row (`hn8qO`, canonical in `LRMQW`) and the column headings above the rows
//! (`K0OMD`). Selecting a row opens the pipeline in the editor.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::PipelinesNav;
use crate::{
  display,
  pipelines::{
    describe::{step_count, task_count, version_state},
    library::{LibraryEntry, VersionState},
  },
  theme::{ExtraColor, ExtraRadius, ExtraSpacing},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{pressable, surface},
    status_label::status_label,
  },
};

/// Column widths and the icon surface (`OY7Sf`, `ATmBR`, `HFjX4`, `cYO90`, `VmMWj`, `hYCdG`,
/// `Lntx7`) and the heading row height (`K0OMD`).
const ICON_WIDTH: f32 = 36.0;
const STEPS_WIDTH: f32 = 56.0;
const VERSION_WIDTH: f32 = 180.0;
const USED_BY_WIDTH: f32 = 64.0;
const PUBLISHED_WIDTH: f32 = 88.0;
const OPEN_WIDTH: f32 = 14.0;
const HEADINGS_HEIGHT: f32 = 32.0;

fn cell(text: &str, width: f32, color: PaletteColor) -> Text {
  Text::new(text)
    .variant(TypographyStyle::Caption)
    .color(color)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .width(Dimension::Px(width))
}

pub fn column_headings(ctx: &Ctx) -> Element {
  let heading = |key: &str, width: f32| cell(&ctx.t(key), width, PaletteColor::TextMuted);
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADINGS_HEIGHT))
    // Inset like the rows, so the columns line up.
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Row::new().width(Dimension::Px(ICON_WIDTH)))
    .child(
      Text::new(&ctx.t("pipelines.library.columns.pipeline"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted)
        .flex(1.0),
    )
    .child(heading("pipelines.library.columns.steps", STEPS_WIDTH))
    .child(heading("pipelines.library.columns.version", VERSION_WIDTH))
    .child(heading("pipelines.library.columns.usedBy", USED_BY_WIDTH))
    .child(heading("pipelines.library.columns.lastPublished", PUBLISHED_WIDTH))
    .child(Row::new().width(Dimension::Px(OPEN_WIDTH)))
    .into()
}

/// "Research → Plan → Implement → Run checks ∥ Review → Create pull request".
fn stages_summary(ctx: &Ctx, stages: &[Vec<std::sync::Arc<str>>]) -> String {
  let (then, parallel) = (ctx.t("pipelines.flow.then"), ctx.t("pipelines.flow.parallel"));
  stages
    .iter()
    .map(|stage| {
      stage
        .iter()
        .map(|name| &**name)
        .collect::<Vec<_>>()
        .join(&format!(" {parallel} "))
    })
    .collect::<Vec<_>>()
    .join(&format!(" {then} "))
}

pub fn library_row(ctx: &Ctx, entry: &LibraryEntry, nav: &PipelinesNav) -> Element {
  let icon_surface = Row::new()
    .size(Dimension::Px(ICON_WIDTH), Dimension::Px(ICON_WIDTH))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(icon(Icon::Workflow, IconSize::Notice, PaletteColor::TextPrimary));
  let summary = if entry.step_count == 0 {
    ctx.t("pipelines.library.noSteps").to_string()
  } else {
    stages_summary(ctx, &entry.stages)
  };
  let identity = Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(Text::new(&entry.name).variant(TypographyStyle::Label))
    .child(
      Text::new(&summary)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let glyph = match entry.state {
    VersionState::Published(_) => Icon::CircleCheck,
    VersionState::DraftOnly | VersionState::PublishedWithDraft(_) => Icon::Pencil,
  };
  let version = Row::new()
    .width(Dimension::Px(VERSION_WIDTH))
    .child(status_label(glyph, &message_text(ctx, &version_state(entry.state))));
  let published = entry.last_published.map_or_else(
    || ctx.t("pipelines.library.never").to_string(),
    |at| message_text(ctx, &display::elapsed(Timestamp::now(), at)).to_string(),
  );
  let (open, id) = (nav.clone(), entry.id);
  let row = surface()
    .id("pipeline-row")
    .describe("pipeline", entry.name.to_string())
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Md)
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
    .child(icon_surface)
    .child(identity)
    .child(cell(
      &message_text(ctx, &step_count(entry.step_count)),
      STEPS_WIDTH,
      PaletteColor::TextSecondary,
    ))
    .child(version)
    .child(cell(
      &message_text(ctx, &task_count(entry.used_by)),
      USED_BY_WIDTH,
      PaletteColor::TextSecondary,
    ))
    .child(cell(&published, PUBLISHED_WIDTH, PaletteColor::TextSecondary))
    .child(icon(Icon::ChevronRight, IconSize::Dismiss, PaletteColor::TextSecondary));
  pressable(row, move || open.open(id)).into()
}
