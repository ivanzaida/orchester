//! The Tests card's failed and flaky tests (`qN2FK`). *Failed* (`iiccE`): 12/600 `text_secondary` and how many, how
//! they ran again and where they went back in 12 `text_muted`, 10 over it and 14 at its sides; then a row per failed
//! test (`BAbAn`), 10 over and under and 14 at its sides under a `border` line, 8 apart: a 13 `circle-x` in `danger`,
//! its name in Geist Mono 12.5/500, its file:line in Geist Mono 12 `text_muted` and its artifacts' links at its end,
//! over the start of its message on `code_bg` in a `border` stroke, radius 6, 8 over and under and 10 at its sides,
//! lines 3 apart in Geist Mono 11.5, the first in `danger` (`K25RNW`). A suite that failed besides its tests is a row
//! of its own, its problem as its message. *Flaky* (`fHKUF`): a row per flaky test, a 13 `refresh-ccw` in `warning`,
//! its name in Geist Mono 12.5, its file:line, and *passed on re-run* (`warning`) and *quarantined* (neutral) badges
//! at its end; a quarantined test that failed again reads *failed again* (`danger`) and *quarantined*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  test_report::MAX_FAILURES_SHOWN,
  test_step::{TestRecord, TestStepRecord},
};

use super::{artifacts, words::counted};
use crate::{
  theme::{ExtraColor, ExtraSpacing, IconSize, TestsText},
  ui::{
    chip::{BadgeTone, badge},
    icon::{Icon, icon},
  },
};

/// The most lines of a failed test's message the card shows.
const MESSAGE_LINES: usize = 4;

/// What failed: *Failed* and its rows; none when nothing did. `returned_to`: the step the failed tests went back to.
pub fn section(ctx: &mut Ctx, record: &TestStepRecord, returned_to: Option<&str>) -> Vec<Element> {
  let failed: Vec<&TestRecord> = record.failed().collect();
  let problems: Vec<(&str, &str)> = record
    .suites()
    .filter_map(|suite| Some((suite.suite.as_str(), suite.problem.as_deref()?)))
    .collect();
  if failed.is_empty() && problems.is_empty() {
    return Vec::new();
  }
  let count = failed.len().to_string();
  let detail = match returned_to {
    Some(step) => ctx.t_args(
      "summary.tests.failedReturned",
      [("count", count.as_str()), ("step", step)],
    ),
    None => ctx.t_args("summary.tests.failedDetail", [("count", count.as_str())]),
  };
  let head = Row::new()
    .id("task-tests-failed-head")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Loose.role())
        .left(ExtraSpacing::Inset.role())
        .right(ExtraSpacing::Inset.role()),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&ctx.t("summary.tests.failedTitle")).variant(TestsText::Heading.role()).nowrap())
    .child(
      Text::new(&detail)
        .id("task-tests-failed-detail")
        .variant(lurq::app::theme::TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  let mut rows: Vec<Element> = vec![head.into()];
  for (index, test) in failed.iter().take(MAX_FAILURES_SHOWN).enumerate() {
    let id = format!("task-tests-failed-{index}");
    let links = artifacts::links(ctx, &id, &test.artifacts);
    rows.push(failed_row(&id, (&test.name, test.location.as_deref()), links, message(test)));
  }
  rows.extend(problems.iter().enumerate().map(|(index, (suite, problem))| {
    failed_row(&format!("task-tests-suite-{index}"), (suite, None), Vec::new(), vec![*problem])
  }));
  let more = failed.len().saturating_sub(MAX_FAILURES_SHOWN);
  if more > 0 {
    let keys = ("summary.tests.moreFailed.one", "summary.tests.moreFailed.other");
    let words = Text::new(&counted(ctx, keys, more))
      .id("task-tests-failed-more")
      .variant(lurq::app::theme::TypographyStyle::Description)
      .color(PaletteColor::TextMuted);
    rows.push(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(ExtraSpacing::Loose.role(), ExtraSpacing::Inset.role()))
        .child(words)
        .into(),
    );
  }
  rows
}

/// The lines of `test`'s message the card shows: the start of its message, else of its excerpt.
fn message(test: &TestRecord) -> Vec<&str> {
  let text = test.message.as_deref().or(test.excerpt.as_deref()).unwrap_or_default();
  text
    .lines()
    .filter(|line| !line.trim().is_empty())
    .take(MESSAGE_LINES)
    .collect()
}

/// A failed test's row, or a failed suite's: its `name` and `location`, its artifacts' `links`, and its message's
/// `lines`.
fn failed_row(id: &str, (name, location): (&str, Option<&str>), links: Vec<Element>, lines: Vec<&str>) -> Element {
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleX, IconSize::Base, PaletteColor::Danger))
    .child(
      Text::new(name)
        .id(format!("{id}-name"))
        .variant(TestsText::Name.role())
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(location.map(|location| {
      Text::new(location)
        .id(format!("{id}-location"))
        .variant(TestsText::Detail.role())
        .min_width(Dimension::Px(0.0))
    }))
    .child(Row::new().flex(1.0))
    .with_children(links);
  let box_lines = lines.iter().enumerate().map(|(index, text)| {
    let line = Text::new(text)
      .id(format!("{id}-message-{index}"))
      .variant(TestsText::Message.role())
      .width(Dimension::Pct(100.0));
    if index == 0 { line.color(PaletteColor::Danger) } else { line }
  });
  let message = (!lines.is_empty()).then(|| {
    Column::new()
      .id(format!("{id}-message"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Fine.role())
      .background(ExtraColor::CodeBackground.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .with_children(box_lines)
  });
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Sm)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(line)
    .with_children(message)
    .into()
}

/// *Flaky* and its rows; `None` when no test was flaky and no quarantined one failed again.
pub fn flaky_section(ctx: &Ctx, record: &TestStepRecord) -> Option<Element> {
  let rerun: Arc<str> = ctx.t("summary.tests.passedOnRerun");
  let quarantined: Arc<str> = ctx.t("summary.tests.quarantined");
  let failed_again: Arc<str> = ctx.t("summary.tests.failedAgain");
  let flaky = record.flaky().map(|test| {
    let mut marks = vec![(BadgeTone::Warning, rerun.clone(), "rerun")];
    if test.quarantined {
      marks.push((BadgeTone::Neutral, quarantined.clone(), "quarantined"));
    }
    (Icon::RefreshCw, PaletteColor::Warning, &test.name, test.location.as_deref(), marks)
  });
  let held = record.quarantined().map(|test| {
    let marks = vec![
      (BadgeTone::Danger, failed_again.clone(), "failed-again"),
      (BadgeTone::Neutral, quarantined.clone(), "quarantined"),
    ];
    (Icon::CircleX, PaletteColor::TextMuted, &test.name, test.location.as_deref(), marks)
  });
  let rows: Vec<Element> = flaky
    .chain(held)
    .enumerate()
    .map(|(index, (glyph, tone, name, location, marks))| {
      let id = format!("task-tests-flaky-{index}");
      let badges = marks
        .into_iter()
        .map(|(badge_tone, word, mark)| Element::from(badge(&format!("{id}-{mark}"), badge_tone, &word)));
      Row::new()
        .id(id.clone())
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        // The design's `refresh-ccw` has no glyph in the bundled `lucide.ttf`: `refresh-cw` draws it.
        .child(icon(glyph, IconSize::Base, tone))
        .child(
          Text::new(name)
            .id(format!("{id}-name"))
            .variant(TestsText::FlakyName.role())
            .min_width(Dimension::Px(0.0)),
        )
        .with_children(location.map(|location| {
          Text::new(location)
            .id(format!("{id}-location"))
            .variant(TestsText::Detail.role())
            .min_width(Dimension::Px(0.0))
        }))
        .child(Row::new().flex(1.0))
        .with_children(badges)
        .into()
    })
    .collect();
  if rows.is_empty() {
    return None;
  }
  let section = Column::new()
    .id("task-tests-flaky")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Sm)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&ctx.t("summary.tests.flakyTitle")).variant(TestsText::Heading.role()))
    .with_children(rows);
  Some(section.into())
}
