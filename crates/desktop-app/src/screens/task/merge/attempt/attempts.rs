//! *Attempts* (`Section · Attempts` in 13e `J2Nbc`): every attempt of the run, newest first, Orchester's retries
//! among them, in one `border` box. A line 36 high says when, who and from where, and how the attempt ended, in
//! `success` once it landed and `warning` else; pressed, it expands into the attempt's own compact checks under it, and
//! its chevron turns up (`EtaSH`).

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::integration::attempt::RecordedAttempt;
use orchester_workspace::display;

use super::{
  checks::{self, Branches, Folds},
  lines::who,
  words::{attempt_outcome, attempt_time, attempt_via},
};
use crate::{
  theme::{ControlHeight, DeliveryText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Expanded {
  /// The attempts open, by their place among the run's attempts.
  pub open: Vec<u32>,
  /// The attempts open whose stages not reached are unfolded.
  pub reached: Vec<u32>,
}

impl DevtoolsInspectable for Expanded {}

/// An attempt line's time column (`LjZ9t`).
const TIME_WIDTH: f32 = 72.0;

/// Whether `attempts` hold two a minute apart or less, which their rows tell apart by the second.
pub(super) fn to_the_second(attempts: &[RecordedAttempt]) -> bool {
  let minutes: Vec<String> = attempts
    .iter()
    .map(|recorded| display::clock(recorded.attempt.at()))
    .collect();
  minutes
    .iter()
    .enumerate()
    .any(|(at, minute)| minutes[at + 1..].contains(minute))
}

/// The list of `attempts` of run `run`, `expanded` saying which show their checks.
pub(super) fn attempts(
  ctx: &mut Ctx,
  (attempts, run): (&[RecordedAttempt], u32),
  branches: &Branches<'_>,
  expanded: &Signal<Expanded>,
) -> Element {
  let seconds = to_the_second(attempts);
  let open = expanded.get().open;
  let mut rows: Vec<Element> = Vec::new();
  for (index, recorded) in attempts.iter().enumerate() {
    let sequence = recorded.sequence;
    let id = format!("merge-attempt-{sequence}");
    let unfolded = open.contains(&sequence);
    let last = index + 1 == attempts.len();
    rows.push(row(
      ctx,
      (&id, recorded, run),
      (unfolded, last && !unfolded),
      (seconds, branches.base),
      expanded,
    ));
    if unfolded {
      let list = checks::checks(
        ctx,
        &format!("{id}-checks"),
        (&recorded.attempt, branches),
        Folds::Attempt(sequence, expanded),
      );
      let mut under = Column::new().width(Dimension::Pct(100.0)).child(list);
      if !last {
        under = under.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
      }
      rows.push(under.into());
    }
  }
  Column::new()
    .id("merge-attempts")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .with_children(rows)
    .into()
}

/// An attempt's row, which expands or folds its checks.
fn row(
  ctx: &Ctx,
  (id, recorded, run): (&str, &RecordedAttempt, u32),
  (unfolded, last): (bool, bool),
  (seconds, base): (bool, &str),
  expanded: &Signal<Expanded>,
) -> Element {
  let attempt = &recorded.attempt;
  let (outcome, color) = attempt_outcome(ctx, attempt, base);
  let (chevron, name) = if unfolded {
    (Icon::ChevronUp, ctx.t("merges.page.attempt.collapse"))
  } else {
    (Icon::ChevronDown, ctx.t("merges.page.attempt.expand"))
  };
  let (expanded, sequence) = (expanded.clone(), recorded.sequence);
  let mut row = pressable(move || {
    expanded.update(|expanded| {
      if let Some(at) = expanded.open.iter().position(|open| *open == sequence) {
        expanded.open.remove(at);
        expanded.reached.retain(|open| *open != sequence);
      } else {
        expanded.open.push(sequence);
      }
    });
  })
  .id(id.to_owned())
  .width(Dimension::Pct(100.0))
  .height(Dimension::Px(ControlHeight::Lg.value()))
  .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
  .spacing(ExtraSpacing::Loose.role())
  .align_items(Alignment::Center)
  .describe("a11y_name", name)
  .describe("expanded", unfolded.to_string())
  .describe("outcome", attempt.outcome().name())
  .child(
    Row::new().width(Dimension::Px(TIME_WIDTH)).child(
      Text::new(&attempt_time(ctx, attempt.at(), seconds))
        .id(format!("{id}-time"))
        .variant(DeliveryText::Count.role())
        .nowrap(),
    ),
  )
  .child(
    Text::new(&who(ctx, attempt.asked_by()))
      .id(format!("{id}-who"))
      .variant(ExtraTypography::Strong.role())
      .nowrap(),
  )
  .child(
    Text::new(&attempt_via(ctx, attempt, run))
      .id(format!("{id}-via"))
      .variant(ExtraTypography::Property.role())
      .color(PaletteColor::TextMuted)
      .nowrap(),
  )
  .child(Column::new().flex(1.0))
  .child(
    Text::new(&outcome)
      .id(format!("{id}-outcome"))
      .variant(ExtraTypography::Property.role())
      .color(color)
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
      .nowrap()
      .text_overflow(TextOverflow::Elipsis),
  )
  .child(icon(chevron, IconSize::Md, PaletteColor::TextMuted));
  if !last {
    row = row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
  }
  row.into()
}
