//! A check's row (`docs/design/pr-ci-status.md`, "Check rows"): its lucide 13 status icon (`circle-check` `success`,
//! `loader-circle` `text_secondary`, `circle-x` `danger`, `circle-dashed` `text_muted` while queued), its name in Geist
//! Mono 12, a failing one's state in 12 `danger`, and *Details ↗* at its end, which opens its page. On the Summary a
//! failing check's log tail follows under its name: its last lines in Geist Mono 11.5 on `code_bg`, radius 6, 8 over
//! and under and 10 at the sides, the last line in `text_primary`; in the Conversation's card the tail's first line
//! follows its name instead, cut to the row. The tail shown is the one the agent gets: colour codes stripped, secrets
//! redacted.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::pull_request::ci::{CiCheck, CiState, redact::agent_log_tail};

use super::actions::open_page;
use crate::{
  services::Services,
  theme::{CiText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    tooltip::Truncated,
  },
};

/// The last lines of a failing check's log the Summary shows; Fix CI sends the last 40.
const SHOWN_TAIL_LINES: usize = 8;

/// How a check's row starts: its icon in its tone.
fn mark(state: CiState) -> (Icon, PaletteColor) {
  match state {
    CiState::Passed => (Icon::CircleCheck, PaletteColor::Success),
    CiState::Skipped => (Icon::CircleCheck, PaletteColor::TextMuted),
    CiState::Running => (Icon::LoaderCircle, PaletteColor::TextSecondary),
    CiState::Queued => (Icon::CircleDashed, PaletteColor::TextMuted),
    CiState::Failed | CiState::Cancelled => (Icon::CircleX, PaletteColor::Danger),
  }
}

/// Where a check's row is: the Summary's card shows a failing one's tail under it, the Conversation's its first line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
  Summary,
  Conversation,
}

/// The row of `check`, the `index`th of the card `card`, with its tail as `place` shows it; `divided`: a hairline over
/// it.
pub fn row(ctx: &mut Ctx, (card, index): (&str, usize), check: &CiCheck, place: Place, divided: bool) -> Element {
  let id = format!("{card}-check-{index}");
  let (glyph, tone) = mark(check.state);
  let failing = matches!(check.state, CiState::Failed | CiState::Cancelled);
  let state_word = match check.state {
    CiState::Failed => Some(("ci.check.failed", PaletteColor::Danger)),
    CiState::Cancelled => Some(("ci.check.cancelled", PaletteColor::Danger)),
    _ => None,
  };
  let tail = check.log_tail.as_deref().map(agent_log_tail);
  let inline = (place == Place::Conversation)
    .then(|| tail.as_deref().and_then(|tail| tail.lines().next()))
    .flatten()
    .map(|line| Truncated::new(format!("{id}-line"), line.to_owned(), ExtraTypography::MonoKey.role()).render(ctx));
  let line = Row::new()
    .id(id.clone())
    .describe("state", check.state.as_str())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, tone))
    .child(
      Text::new(&check.name)
        .id(format!("{id}-name"))
        .variant(ExtraTypography::PropertyMono.role())
        .nowrap(),
    )
    .with_children(state_word.map(|(key, color)| {
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::Description)
        .color(color)
        .nowrap()
    }))
    .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)).with_children(inline))
    .with_children((check.state == CiState::Queued).then(|| {
      Text::new(&ctx.t("ci.check.queued"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
    }))
    .with_children(details(ctx, &id, check));
  let shown_tail = (place == Place::Summary && failing).then(|| tail_box(ctx, &id, tail.as_deref()));
  let mut column = Column::new()
    .id(format!("{id}-row"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .padding(Padding::symmetric(SpacingSize::Xs, ExtraSpacing::Snug.role()))
    .child(line)
    .with_children(shown_tail);
  if divided {
    column = column.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border));
  }
  column.into()
}

/// *Details ↗*, which opens the check's page, when it has one.
fn details(ctx: &mut Ctx, id: &str, check: &CiCheck) -> Option<Element> {
  let url: Arc<str> = check.url.as_deref()?.into();
  let open = open_page(&Services::of(ctx), url);
  Some(
    ActionButton::new(format!("{id}-details"), ButtonKind::Ghost, ctx.t("ci.details"))
      .after(Icon::ArrowUpRight)
      .small()
      .tight()
      .on_click(open)
      .into(),
  )
}

/// A failing check's log tail as the agent reads it, its last [`SHOWN_TAIL_LINES`] lines; or that none could be read.
fn tail_box(ctx: &Ctx, id: &str, tail: Option<&str>) -> Element {
  let lines: Vec<&str> = tail.map(|tail| tail.lines().collect()).unwrap_or_default();
  let shown = &lines[lines.len().saturating_sub(SHOWN_TAIL_LINES)..];
  let last = shown.len().saturating_sub(1);
  let texts: Vec<Element> = if shown.is_empty() {
    vec![
      Text::new(&ctx.t("ci.check.noLog"))
        .variant(CiText::LogTail.role())
        .color(PaletteColor::TextMuted)
        .into(),
    ]
  } else {
    shown
      .iter()
      .enumerate()
      .map(|(index, line)| {
        let text = Text::new(line)
          .variant(CiText::LogTail.role())
          .width(Dimension::Pct(100.0));
        if index == last {
          text.color(PaletteColor::TextPrimary)
        } else {
          text
        }
        .into()
      })
      .collect()
  };
  Column::new()
    .id(format!("{id}-tail"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(SpacingSize::Xl))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
        .background(ExtraColor::CodeBackground.role())
        .rounded(RadiusSize::Md)
        .with_children(texts),
    )
    .into()
}

/// *3 passed · build, test (windows-latest), e2e*: the checks that passed or were skipped, folded into one line.
pub fn passed_fold(ctx: &Ctx, id: &str, passed: &[&CiCheck]) -> Element {
  let names: Vec<&str> = passed.iter().map(|check| check.name.as_str()).collect();
  let words = ctx.t_args(
    "ci.passedFold",
    [
      ("count", passed.len().to_string()),
      ("names", names.join(&ctx.t("task.listSeparator"))),
    ],
  );
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Xs, ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::CircleCheck, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .id(format!("{id}-text"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
