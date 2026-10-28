//! A new conversation (`03 New conversation` `c99rep`, `Start` `Y7fviC`): centered, 640 wide, 20 apart: a 36 tile
//! (`surface_raised` in a `border` stroke, radius 8) with an 18 `waypoints`; *What should we plan?* (24/600); what the
//! chat is for (13 `text_secondary`); the composer, with the focus; and four suggestions, two a row 6 apart (`border`
//! stroke, radius 8, 8 by 10 inside: a 13 icon in `text_muted` and the prompt in 12 `text_secondary`), which fill the
//! prompt and send nothing.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::View;
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchestratorSpacing, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Truncated,
  },
};

/// The start's width (`Y7fviC`), its tile's side (`x5G1nb`) and a suggestion's height (`VLqDm`).
const WIDTH: f32 = 640.0;
const TILE_SIZE: f32 = 36.0;
const SUGGESTION_HEIGHT: f32 = 32.0;

/// The suggestions, two a row: their icons and words.
const SUGGESTIONS: [[(Icon, &str); 2]; 2] = [
  [
    (Icon::Sparkles, "orchestrator.suggestions.plan"),
    (Icon::ListChecks, "orchestrator.suggestions.running"),
  ],
  [
    (Icon::Sparkles, "orchestrator.suggestions.order"),
    (Icon::ListChecks, "orchestrator.suggestions.idea"),
  ],
];

/// The start around `composer`, with `notice` over the composer when something keeps the chat from sending.
pub(super) fn start(ctx: &mut Ctx, view: &View, (composer, notice): (Column, Option<Element>)) -> Element {
  let tile = Row::new()
    .size(Dimension::Px(TILE_SIZE), Dimension::Px(TILE_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(icon(Icon::Waypoints, IconSize::Xl, PaletteColor::TextSecondary));
  let start = Column::new()
    .id("orchestrator-start")
    .width(Dimension::Px(WIDTH))
    .max_width(Dimension::Pct(100.0))
    .spacing(OrchestratorSpacing::Start.role())
    .align_items(Alignment::Center)
    .child(tile)
    .child(
      Text::new(&ctx.t("orchestrator.start.title"))
        .id("orchestrator-start-title")
        .variant(OrchestratorText::StartTitle.role())
        .text_align(TextAlign::Center),
    )
    .child(
      Text::new(&ctx.t("orchestrator.start.description"))
        .id("orchestrator-start-description")
        .variant(ExtraTypography::Lead.role())
        .text_align(TextAlign::Center)
        .width(Dimension::Pct(100.0)),
    )
    .with_children(notice)
    .child(composer)
    .child(suggestions(ctx, view));
  Column::new()
    .id("orchestrator-new")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(
      Padding::new()
        .left(OrchestratorSpacing::PageSide.role())
        .right(OrchestratorSpacing::PageSide.role())
        .bottom(SpacingSize::Section),
    )
    .align_items(Alignment::Center)
    .child(Spacer::new().flex(1.0))
    .child(start)
    .child(Spacer::new().flex(1.0))
    .into()
}

/// The suggestions, two a row.
fn suggestions(ctx: &mut Ctx, view: &View) -> Element {
  let rows = SUGGESTIONS.map(|row| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .with_children(row.map(|(glyph, key)| suggestion(ctx, view, glyph, key)))
  });
  Column::new()
    .id("orchestrator-suggestions")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .with_children(rows)
    .into()
}

/// One suggestion: pressing it puts its words in the prompt, with the focus. Its words keep to one line, cut
/// where a narrow window leaves them no room, so every suggestion is as high as the design's.
fn suggestion(ctx: &mut Ctx, view: &View, glyph: Icon, key: &str) -> Element {
  let words: Arc<str> = ctx.t(key);
  let (draft, focus) = (view.draft.clone(), view.focus.clone());
  let fill = words.clone();
  let name = key.rsplit('.').next().unwrap_or(key);
  let text = Truncated::new(
    format!("orchestrator-suggestion-{name}-words"),
    words.clone(),
    OrchestratorText::Suggestion.role(),
  )
  .render(ctx);
  pressable(move || {
    draft.set(fill.to_string());
    focus.set(true);
  })
  .id(format!("orchestrator-suggestion-{name}"))
  .describe("a11y_name", words)
  .flex(1.0)
  .min_width(Dimension::Px(0.0))
  .height(Dimension::Px(SUGGESTION_HEIGHT))
  .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
  .spacing(SpacingSize::Sm)
  .align_items(Alignment::Center)
  .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
  .rounded(ExtraRadius::Tile.role())
  .hovered_style(Style::new().background(ExtraColor::Hover.role()))
  .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
  .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(text))
  .into()
}
