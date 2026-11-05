//! An attempt's checks (`Checks` `qfYTz` in 09b; `Attempt 12:41 · checks` `D0SwP` in 09h): the stages in order, each
//! with its number, its glyph, its name, what it touches and how it went, a `border` line under each but the last. The
//! stage that refused says its cause in Geist Mono on `code_bg`, selectable (`vJyj2`); the stages after it were not
//! reached. On the page, two or more checks before Git that passed first fold into one row, *6 checks passed*, with
//! *Show* (`Qqfhg` in 09f), even when every stage passed (09h); *Merge* and *Record* keep their own rows (09g). In an
//! expanded attempt every stage has its own compact row, 12 high.

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

use super::words::{stage_name, stage_result, stage_where};
use crate::{
  theme::{AttemptSpacing, DeliveryText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// How the checks are drawn: on the page, or compact inside an expanded attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Size {
  Page,
  Compact,
}

/// What the checks name besides the attempt: the branch the run merges into and its own branch.
pub(super) struct Branches<'a> {
  pub base: &'a str,
  pub branch: &'a str,
}

/// The checks of `attempt`, `<id>-<n>` for stage `n` (from 1). On the page, `shown` says whether the stages that passed
/// first are unfolded.
pub(super) fn checks(
  ctx: &mut Ctx,
  id: &str,
  (attempt, branches): (&MergeAttempt, &Branches<'_>),
  (size, shown): (Size, Option<&Signal<bool>>),
) -> Element {
  let stages = attempt.stages();
  let passed_first = stages
    .iter()
    .take_while(|(stage, result)| *result == StageResult::Passed && before_git(*stage))
    .count();
  let folds = size == Size::Page && passed_first >= 2;
  let unfolded = shown.is_some_and(|shown| shown.get());
  let mut rows: Vec<Element> = Vec::new();
  if folds {
    rows.push(fold(ctx, id, &stages[..passed_first], (unfolded, shown)));
  }
  for (index, stage) in stages.iter().enumerate() {
    if folds && index < passed_first && !unfolded {
      continue;
    }
    let row = Check {
      id: format!("{id}-{}", index + 1),
      number: index + 1,
      stage: *stage,
      size,
    };
    rows.push(row.render(ctx, attempt, branches, index + 1 == stages.len()));
  }
  let list = Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .describe("folded", (folds && !unfolded).to_string());
  match size {
    Size::Page => list
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .clip()
      .with_children(rows)
      .into(),
    Size::Compact => list
      .padding(
        Padding::new()
          .top(SpacingSize::Xs)
          .bottom(SpacingSize::Sm)
          .left(AttemptSpacing::Indent.role()),
      )
      .with_children(rows)
      .into(),
  }
}

/// *01–06 · 6 checks passed · their names · Show*: the stages that passed first, folded.
fn fold(
  ctx: &mut Ctx,
  id: &str,
  passed: &[(AttemptStage, StageResult)],
  (unfolded, shown): (bool, Option<&Signal<bool>>),
) -> Element {
  let numbers = format!("{:02}–{:02}", 1, passed.len());
  let names: Vec<String> = passed
    .iter()
    .map(|(stage, _)| stage_name(ctx, *stage).to_string())
    .collect();
  let label = if unfolded {
    ctx.t("merges.page.hide")
  } else {
    ctx.t("merges.page.show")
  };
  let shown = shown.cloned();
  pressable(move || {
    if let Some(shown) = &shown {
      shown.update(|shown| *shown = !*shown);
    }
  })
  .id(format!("{id}-passed"))
  .width(Dimension::Pct(100.0))
  .padding(Padding::symmetric(SpacingSize::Lg, AttemptSpacing::CheckY.role()))
  .spacing(SpacingSize::Md)
  .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
  .describe("a11y_name", label.clone())
  .child(Text::new(&numbers).variant(DeliveryText::Count.role()).nowrap())
  .child(icon(Icon::CircleCheck, IconSize::Md, PaletteColor::Success))
  .child(
    Text::new(&ctx.t_args("merges.page.passed", [("count", passed.len().to_string())]))
      .variant(TypographyStyle::Body)
      .color(PaletteColor::TextSecondary)
      .nowrap(),
  )
  .child(
    Text::new(&names.join(&ctx.t("merges.separator")))
      .id(format!("{id}-passed-names"))
      .variant(ExtraTypography::Property.role())
      .color(PaletteColor::TextMuted)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .nowrap()
      .text_overflow(TextOverflow::Elipsis),
  )
  .child(
    Text::new(&label)
      .id(format!("{id}-passed-toggle"))
      .variant(ExtraTypography::Property.role())
      .color(PaletteColor::TextMuted)
      .nowrap(),
  )
  .into()
}

/// One stage's row.
struct Check {
  id: String,
  number: usize,
  stage: (AttemptStage, StageResult),
  size: Size,
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
    let line = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
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
    let y = match self.size {
      Size::Page => AttemptSpacing::CheckY.role(),
      Size::Compact => AttemptSpacing::CompactY.role(),
    };
    let mut row = Row::new()
      .id(self.id.clone())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, y))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .describe("stage", stage.name())
      .describe("result", result_name(result))
      .child(Text::new(&format!("{:02}", self.number)).variant(DeliveryText::Count.role()))
      .child(icon(glyph, IconSize::Md, color))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Snug.role())
          .child(line)
          .with_children(cause),
      );
    if !last && self.size == Size::Page {
      row = row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
    }
    row.into()
  }

  /// The refused stage's name stands out: 13/500 on the page, 12/500 in an expanded attempt.
  fn name_style(&self, refused: bool) -> TypographyStyle {
    match (self.size, refused) {
      (Size::Page, true) => ExtraTypography::Strong.role(),
      (Size::Page, false) => TypographyStyle::Body,
      (Size::Compact, true) => TypographyStyle::Label,
      (Size::Compact, false) => ExtraTypography::Property.role(),
    }
  }
}

/// The refused stage's cause, as the OS, Git or Orchester said it: Geist Mono on `code_bg`, selectable.
fn cause_box(id: &str, cause: &str) -> Element {
  Row::new()
    .id(format!("{id}-cause"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .background(ExtraColor::CodeBackground.role())
    .rounded(RadiusSize::Sm)
    .child(
      Text::new(cause)
        .id(format!("{id}-cause-text"))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary)
        .selectable(true)
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// Whether `stage` is one of the checks before Git: only those fold; *Merge* and *Record* always keep their own rows,
/// so the commit Git made and whether it was recorded stay in sight (09g, 09h).
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
