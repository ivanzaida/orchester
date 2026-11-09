//! An attempt's checks (`Section · Checks` in 13c `zWlkp`; an attempt's own in 13e `J2Nbc`): the stages in order, each
//! a line 36 high (30 in an expanded attempt) with its number in a 40 column, its glyph, its name, what it touches and
//! how it went, a `border` line under each but the last. The stage that refused says its cause in Geist Mono on
//! `code_bg` under it, selectable, in line with its glyph; the stages after it were not reached. On the page, two or
//! more checks before Git that passed first fold into one row, *6 checks passed*, with *Show*, even when every stage
//! passed (13e); *Merge* and *Record* keep their own rows (13d). In an expanded attempt the stages not reached fold
//! into one row.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::integration::attempt::{AttemptStage, MergeAttempt, StageResult};

use super::{
  attempts::Expanded,
  words::{stage_name, stage_result, stage_where},
};
use crate::{
  theme::{ControlHeight, DeliveryText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// A compact line's height, in an expanded attempt (`M9evI` in 13e).
const COMPACT_HEIGHT: f32 = 30.0;
/// A stage's number column (`Number`), and the room a cause leaves for it, its glyph and the gaps between: it starts in
/// line with the glyph's start (`D9NcH`).
pub(super) const NUMBER_WIDTH: f32 = 40.0;
const CAUSE_INDENT: f32 = 64.0;

/// How the checks are drawn, and what they fold.
#[derive(Clone, Copy)]
pub(super) enum Folds<'a> {
  /// On the page: whether the stages that passed first are unfolded.
  Page(&'a Signal<bool>),
  /// In the expanded attempt `sequence`: the stages not reached fold, `expanded` says whether they are unfolded.
  Attempt(u32, &'a Signal<Expanded>),
}

impl Folds<'_> {
  const fn compact(&self) -> bool {
    matches!(self, Self::Attempt(..))
  }

  fn unfolded(&self) -> bool {
    match self {
      Self::Page(shown) => shown.get(),
      Self::Attempt(sequence, expanded) => expanded.get().reached.contains(sequence),
    }
  }

  fn toggle(&self) -> Box<dyn Fn() + Send + Sync> {
    match self {
      Self::Page(shown) => {
        let shown = (*shown).clone();
        Box::new(move || shown.update(|shown| *shown = !*shown))
      }
      Self::Attempt(sequence, expanded) => {
        let (sequence, expanded) = (*sequence, (*expanded).clone());
        Box::new(move || {
          expanded.update(
            |expanded| match expanded.reached.iter().position(|open| *open == sequence) {
              Some(at) => {
                expanded.reached.remove(at);
              }
              None => expanded.reached.push(sequence),
            },
          );
        })
      }
    }
  }
}

/// What the checks name besides the attempt: the branch the run merges into and its own branch.
pub(super) struct Branches<'a> {
  pub base: &'a str,
  pub branch: &'a str,
}

/// The stages that fold: those that passed first, on the page; those not reached, in an expanded attempt.
fn fold_of(stages: &[(AttemptStage, StageResult)], folds: &Folds<'_>) -> Option<(usize, usize)> {
  let (start, result) = if folds.compact() {
    let reached = stages
      .iter()
      .rposition(|(_, result)| *result != StageResult::NotReached)
      .map_or(0, |last| last + 1);
    (reached, StageResult::NotReached)
  } else {
    (0, StageResult::Passed)
  };
  let len = stages[start..]
    .iter()
    .take_while(|(stage, stage_result)| {
      *stage_result == result && (result == StageResult::NotReached || before_git(*stage))
    })
    .count();
  (len >= 2).then_some((start, len))
}

/// The checks of `attempt`, `<id>-<n>` for stage `n` (from 1).
pub(super) fn checks(
  ctx: &mut Ctx,
  id: &str,
  (attempt, branches): (&MergeAttempt, &Branches<'_>),
  folds: Folds<'_>,
) -> Element {
  let stages = attempt.stages();
  let fold = fold_of(stages, &folds);
  let unfolded = folds.unfolded();
  let shown: Vec<usize> = (0..stages.len())
    .filter(|index| {
      fold
        .as_ref()
        .is_none_or(|(start, len)| unfolded || !(*start..start + len).contains(index))
    })
    .collect();
  let count = shown.len() + usize::from(fold.is_some());
  let mut rows: Vec<Element> = Vec::new();
  let mut place = 0;
  for index in 0..stages.len() {
    if let Some((start, len)) = &fold
      && index == *start
    {
      place += 1;
      rows.push(fold_row(
        ctx,
        (id, &stages[*start..start + len], *start),
        (&folds, unfolded),
        place == count,
      ));
    }
    if shown.contains(&index) {
      place += 1;
      let row = Check {
        id: format!("{id}-{}", index + 1),
        number: index + 1,
        stage: stages[index],
        compact: folds.compact(),
      };
      rows.push(row.render(ctx, attempt, branches, place == count));
    }
  }
  let list = Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .describe("folded", (fold.is_some() && !unfolded).to_string());
  if folds.compact() {
    list
      .background(ExtraColor::CodeBackground.role())
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Tight.role())
          .bottom(ExtraSpacing::Tight.role())
          .left(EXPANDED_INDENT),
      )
      .with_children(rows)
      .into()
  } else {
    list
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .clip()
      .with_children(rows)
      .into()
  }
}

/// An expanded attempt's checks in from the box's side (`EtaSH`).
const EXPANDED_INDENT: f32 = 82.0;

/// One line of a check: `height` high, its parts 10 apart.
pub(super) fn line(compact: bool) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(if compact {
      COMPACT_HEIGHT
    } else {
      ControlHeight::Lg.value()
    }))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
}

/// A check's box: its line under its `border`, unless it is the last.
pub(super) fn framed(id: String, last: bool) -> Column {
  let column = Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()));
  if last {
    column
  } else {
    column.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
  }
}

/// *01–06 · 6 checks passed · their names · Show*: the stages that passed first, or the stages not reached, folded.
fn fold_row(
  ctx: &mut Ctx,
  (id, folded, start): (&str, &[(AttemptStage, StageResult)], usize),
  (folds, unfolded): (&Folds<'_>, bool),
  last: bool,
) -> Element {
  let compact = folds.compact();
  let numbers = format!("{:02}–{:02}", start + 1, start + folded.len());
  let names: Vec<String> = folded
    .iter()
    .map(|(stage, _)| stage_name(ctx, *stage).to_string())
    .collect();
  let label = if unfolded {
    ctx.t("merges.page.hide")
  } else {
    ctx.t("merges.page.show")
  };
  let passed = folded.first().is_some_and(|(_, result)| *result == StageResult::Passed);
  let (glyph, color, words) = if passed {
    (
      Icon::CircleCheck,
      PaletteColor::Success,
      ctx.t_args("merges.page.passed", [("count", folded.len().to_string())]),
    )
  } else {
    (
      Icon::CircleDashed,
      PaletteColor::TextMuted,
      ctx.t_args("merges.page.notReached", [("count", folded.len().to_string())]),
    )
  };
  let suffix = if passed { "passed" } else { "not-reached" };
  let toggle = folds.toggle();
  let line = line(compact)
    .child(
      Row::new()
        .width(Dimension::Px(NUMBER_WIDTH))
        .child(Text::new(&numbers).variant(DeliveryText::Count.role()).nowrap()),
    )
    .child(icon(glyph, IconSize::Md, color))
    .child(
      Text::new(&words)
        .variant(if compact {
          ExtraTypography::Property.role()
        } else {
          TypographyStyle::Body
        })
        .color(if passed && !compact {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextSecondary
        })
        .nowrap(),
    )
    .child(
      Text::new(&names.join(&ctx.t("merges.separator")))
        .id(format!("{id}-{suffix}-names"))
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(
      Text::new(&label)
        .id(format!("{id}-{suffix}-toggle"))
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    );
  let row = pressable(toggle)
    .id(format!("{id}-{suffix}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .describe("a11y_name", label)
    .child(line);
  if last {
    row.into()
  } else {
    row
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  }
}

/// One stage's row.
struct Check {
  id: String,
  number: usize,
  stage: (AttemptStage, StageResult),
  compact: bool,
}

impl Check {
  fn render(self, ctx: &Ctx, attempt: &MergeAttempt, branches: &Branches<'_>, last: bool) -> Element {
    let (stage, result) = self.stage;
    let (glyph, color) = match result {
      StageResult::Passed => (Icon::CircleCheck, PaletteColor::Success),
      StageResult::Refused => (Icon::CircleX, PaletteColor::Warning),
      StageResult::NotReached => (Icon::CircleDashed, PaletteColor::TextMuted),
    };
    let (said, said_color) = stage_result(ctx, attempt, self.stage);
    let refused = result == StageResult::Refused;
    let cause = attempt
      .cause()
      .filter(|cause| refused && !cause.trim().is_empty())
      .map(|cause| cause_box(&self.id, cause));
    let name = Text::new(&stage_name(ctx, stage))
      .id(format!("{}-name", self.id))
      .variant(self.name_style(refused))
      .color(if refused {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextSecondary
      })
      .nowrap();
    let line = line(self.compact)
      .child(
        Row::new()
          .width(Dimension::Px(NUMBER_WIDTH))
          .child(Text::new(&format!("{:02}", self.number)).variant(DeliveryText::Count.role())),
      )
      .child(icon(glyph, IconSize::Md, color))
      .child(name)
      .child(
        Text::new(&stage_where(ctx, stage, (branches.base, branches.branch)))
          .variant(DeliveryText::Count.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .child(
        Text::new(&said)
          .id(format!("{}-result", self.id))
          .variant(ExtraTypography::Property.role())
          .color(said_color)
          .nowrap(),
      );
    framed(self.id, last)
      .describe("stage", stage.name())
      .describe("result", result_name(result))
      .child(line)
      .with_children(cause)
      .into()
  }

  /// The refused stage's name stands out: 13/500 on the page; an expanded attempt's names are 12.
  fn name_style(&self, refused: bool) -> TypographyStyle {
    match (self.compact, refused) {
      (false, true) => ExtraTypography::Strong.role(),
      (false, false) => TypographyStyle::Body,
      (true, _) => ExtraTypography::Property.role(),
    }
  }
}

/// The refused stage's cause, as the OS, Git or Orchester said it: Geist Mono on `code_bg`, selectable, 64 in from its
/// check's start and 10 above the line under it.
fn cause_box(id: &str, cause: &str) -> Element {
  Row::new()
    .id(format!("{id}-cause"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(CAUSE_INDENT).bottom(ExtraSpacing::Loose.role()))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
        .background(ExtraColor::CodeBackground.role())
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Sm)
        .child(
          Text::new(cause)
            .id(format!("{id}-cause-text"))
            .variant(TypographyStyle::Mono)
            .color(PaletteColor::TextSecondary)
            .selectable(true)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}

/// Whether `stage` is one of the checks before Git: only those fold when they passed; *Merge* and *Record* always keep
/// their own rows, so the commit Git made and whether it was recorded stay in sight (13d, 13e).
const fn before_git(stage: AttemptStage) -> bool {
  !matches!(stage, AttemptStage::Merge | AttemptStage::Record)
}

const fn result_name(result: StageResult) -> &'static str {
  match result {
    StageResult::Passed => "passed",
    StageResult::Refused => "refused",
    StageResult::NotReached => "not_reached",
  }
}

/// The *Checks* heading's count: *attempt 2 · 12:41*.
pub(super) fn heading_count(ctx: &Ctx, (number, time): (u32, Arc<str>)) -> Arc<str> {
  ctx.t_args(
    "merges.page.checksOf",
    [("number", number.to_string()), ("time", time.to_string())],
  )
}
