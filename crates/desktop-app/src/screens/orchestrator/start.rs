//! A new conversation (`Orchestrator · new conversation` `HvzWI`, `Body` `V6ZLzQ`): centered, 680 wide, 20 apart:
//! *What should we plan?* (28/600) with what the chat is for (12 `text_secondary`, 1.5); the composer, its box three
//! lines high, with the keys and what each reply gets under it; the suggestions (`Prompt chip` `gFvYI` on `hover`,
//! radius 8, 8 by 10 inside: a 13 icon, the prompt in 12 `text_secondary` and a 12 `corner-down-left`), two a row,
//! which fill the prompt and send nothing; and at the foot where conversations stay.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::View;
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchestratorSpacing, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    pressable::{inert, pressable},
    tooltip::Tooltip,
  },
};

/// The start's width (`Start` `ktXkH`) and a suggestion's height (`IMKR2`).
const WIDTH: f32 = 680.0;
const SUGGESTION_HEIGHT: f32 = 52.0;

/// The suggestions, two a row: their icons and words.
const SUGGESTIONS: [[(Icon, &str); 2]; 2] = [
  [
    (Icon::ListTodo, "orchestrator.suggestions.plan"),
    (Icon::Activity, "orchestrator.suggestions.running"),
  ],
  [
    (Icon::ListOrdered, "orchestrator.suggestions.order"),
    (Icon::Lightbulb, "orchestrator.suggestions.idea"),
  ],
];

/// The start around `composer` (the box and the keys under it) for project `project`, with `notice` over the
/// composer when something keeps the chat from sending.
pub(super) fn start(
  ctx: &mut Ctx,
  view: &View,
  (composer, notice): (Column, Option<Element>),
  project: &str,
) -> Element {
  let intro = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&ctx.t("orchestrator.start.title"))
        .id("orchestrator-start-title")
        .variant(ExtraTypography::Display.role()),
    )
    .child(
      Text::new(&ctx.t("orchestrator.start.description"))
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    );
  let under = Row::new()
    .id("orchestrator-under-composer")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(super::composer::keys(ctx))
    .child(Spacer::new().flex(1.0))
    .child(icon(Icon::Layers2, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t_args("orchestrator.context.line", [("project", project)]))
        .variant(TypographyStyle::Caption)
        .flex_shrink(1.0)
        .nowrap(),
    )
    .child(manage_context(ctx));
  let start = Column::new()
    .id("orchestrator-start")
    .width(Dimension::Px(WIDTH))
    .max_width(Dimension::Pct(100.0))
    .spacing(OrchestratorSpacing::Start.role())
    .child(intro)
    .with_children(notice)
    .child(composer.child(under))
    .child(suggestions(ctx, view));
  Column::new()
    .id("orchestrator-new")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(
      Padding::new()
        .left(SpacingSize::Section)
        .right(SpacingSize::Section)
        .bottom(SpacingSize::Lg),
    )
    .align_items(Alignment::Center)
    .child(Spacer::new().flex(1.0))
    .child(start)
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&ctx.t_args("orchestrator.footer", [("project", project)]))
        .id("orchestrator-footer")
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .into()
}

/// *Manage context*, not built: its words and a lock, why in its tooltip.
fn manage_context(ctx: &mut Ctx) -> Element {
  let reason = ctx.t("orchestrator.manageContextUnavailable");
  let link = inert(lurq::components::Button::empty())
    .id("orchestrator-context-link")
    .describe("state", "unavailable")
    .describe("reason", reason.clone())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("orchestrator.manageContext"))
        .variant(OrchestratorText::ContextLink.role())
        .nowrap(),
    )
    .child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted));
  Tooltip::new("orchestrator-context-link", reason).wrap(ctx, link)
}

/// The suggestions, their heading and what pressing one does.
fn suggestions(ctx: &mut Ctx, view: &View) -> Element {
  let rows = SUGGESTIONS.map(|row| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(row.map(|(glyph, key)| suggestion(ctx, view, glyph, key)))
  });
  Column::new()
    .id("orchestrator-suggestions")
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("orchestrator.suggestions.eyebrow")).variant(OrchestratorText::Eyebrow.role()))
    .with_children(rows)
    .child(
      Text::new(&ctx.t("orchestrator.suggestions.note"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// One suggestion: pressing it puts its words in the prompt, with the focus.
fn suggestion(ctx: &mut Ctx, view: &View, glyph: Icon, key: &str) -> Element {
  let words: Arc<str> = ctx.t(key);
  let (draft, focus) = (view.draft.clone(), view.focus.clone());
  let fill = words.clone();
  pressable(move || {
    draft.set(fill.to_string());
    focus.set(true);
  })
  .id(format!(
    "orchestrator-suggestion-{}",
    key.rsplit('.').next().unwrap_or(key)
  ))
  .describe("a11y_name", words.clone())
  .flex(1.0)
  .height(Dimension::Px(SUGGESTION_HEIGHT))
  .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
  .spacing(ExtraSpacing::Inline.role())
  .align_items(Alignment::Start)
  .background(ExtraColor::Hover.role())
  .rounded(ExtraRadius::Tile.role())
  .hovered_style(Style::new().background(ExtraColor::Pressed.role()))
  .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
  .child(
    Text::new(&words)
      .variant(OrchestratorText::Suggestion.role())
      .flex(1.0)
      .min_width(Dimension::Px(0.0)),
  )
  .child(icon(Icon::CornerDownLeft, IconSize::Sm, PaletteColor::TextMuted))
  .into()
}
