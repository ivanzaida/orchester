//! The decision card (`3 · Decision card` `bxJZp` in `Orchestrator cards · redesign` `zuvUi`): only for what a person
//! decides. `surface_raised` in a 1 `border_strong` stroke (`warning` when it widens what agents may do), radius 10,
//! 12 over and under and 14 at its sides, its parts 10 apart. Its head: a 14 icon, the question (13/600), the target
//! in Geist Mono 11.5 `text_muted` and at its end the *Waits for you · time* chip; then what it is about (12.5
//! `text_secondary`), its facts, a warning strip only for what is risky in it (`warning_muted`, radius 6, 8 by 10
//! inside, a 13 `triangle-alert` and 12 `text_secondary`), and its actions: the accept label as the primary button
//! (danger for a cancel), *Dismiss* as a ghost, an optional place to open, and at the end a lock and *Only you can
//! apply*. Decided, the card folds into an event line ([`super::line`]).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::line::{Chip, ChipTone, chip_element};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// How many facts show before *Show details*.
pub(super) const SHOWN_FACTS: usize = 3;
/// A fact's key column (`yhR7Q` in `bxJZp`).
pub(super) const FACT_KEY_WIDTH: f32 = 96.0;

/// The card's frame, named `id`; `outcome` is what tools read of it, and `widens` strokes it in `warning`.
pub(super) fn frame(id: &str, outcome: &'static str, widens: bool) -> Column {
  let stroke = if widens {
    PaletteColor::Warning
  } else {
    PaletteColor::BorderStrong
  };
  Column::new()
    .id(id.to_owned())
    .describe("outcome", outcome)
    .describe("widens", widens.to_string())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Lg)
}

/// The card's head: `glyph`, the question, the target and, at its end, the chip saying it waits for the person since
/// `waits`.
pub(super) fn head(ctx: &mut Ctx, id: &str, (glyph, question): (Icon, &str), target: Option<&str>, waits: &str) -> Row {
  let chip = Chip::new(
    ctx.t_args("orchestrator.proposal.waits", [("time", waits)]),
    ChipTone::Warning,
  );
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary).flex_shrink(0.0))
    .child(
      Text::new(question)
        .id(format!("{id}-title"))
        .variant(OrchestratorText::Question.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(target.filter(|target| !target.is_empty()).map(|target| {
      Text::new(target)
        .id(format!("{id}-target"))
        .variant(ExtraTypography::MonoKey.role())
        .nowrap()
        .flex_shrink(0.0)
    }))
    .child(Spacer::new().flex(1.0))
    .child(chip_element(ctx, &format!("{id}-state"), &chip))
}

/// What the card is about, in 12.5 `text_secondary`: `{id}-effect`.
pub(super) fn body(id: &str, words: &str) -> Text {
  Text::new(words)
    .id(format!("{id}-effect"))
    .variant(ExtraTypography::Lead.role())
    .width(Dimension::Pct(100.0))
}

/// A warning strip of card `id` (`{id}-{suffix}`): only for what is risky in it.
pub(super) fn warning(id: &str, suffix: &str, words: &str) -> Row {
  Row::new()
    .id(format!("{id}-{suffix}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .background(PaletteColor::WarningMuted)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::TriangleAlert, IconSize::Base, PaletteColor::Warning).flex_shrink(0.0))
    .child(
      Text::new(words)
        .variant(ExtraTypography::Prose.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// *Show details* or *Hide details* of card `id`, which opens the facts after the first [`SHOWN_FACTS`].
pub(super) fn details_toggle(ctx: &Ctx, id: &str, open: &Signal<bool>) -> ActionButton {
  let shown = open.get();
  let toggle = open.clone();
  ActionButton::new(
    format!("{id}-details"),
    ButtonKind::Ghost,
    ctx.t(if shown {
      "orchestrator.decision.hideDetails"
    } else {
      "orchestrator.decision.showDetails"
    }),
  )
  .icon(if shown { Icon::ChevronUp } else { Icon::ChevronDown })
  .small()
  .on_click(move || toggle.update(|open| *open = !*open))
}

/// The card's actions row, `{id}-actions`: `buttons` (the accept, *Dismiss*, a place to open), then at its end a lock
/// and *Only you can apply*.
pub(super) fn actions(ctx: &Ctx, id: &str, buttons: Vec<Element>) -> Row {
  Row::new()
    .id(format!("{id}-actions"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(buttons)
    .child(Spacer::new().flex(1.0))
    .child(
      Row::new()
        .id(format!("{id}-person"))
        .flex_shrink(0.0)
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t("orchestrator.decision.onlyYou"))
            .variant(ExtraTypography::Note.role())
            .nowrap(),
        ),
    )
}

/// *Dismiss*, as the cards' ghost.
pub(super) fn dismiss_button(ctx: &Ctx, id: &str) -> ActionButton {
  ActionButton::new(
    format!("{id}-dismiss"),
    ButtonKind::Ghost,
    ctx.t("orchestrator.proposal.dismiss"),
  )
}

/// A place the card opens, as a ghost with `arrow-up-right`, named `{id}-open` or `{id}-open-{index}`.
pub(super) fn open_button(id: String, label: Arc<str>) -> ActionButton {
  ActionButton::new(id, ButtonKind::Ghost, label).icon(Icon::ArrowUpRight)
}
