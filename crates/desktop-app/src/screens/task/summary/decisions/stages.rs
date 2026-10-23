//! The run's stages on one line (`Stages` `cRN0U`), a chip per step (`Stage chip` `hDyRI`): its state's
//! icon, its number and its name in a pill. The step the decision is about is drawn in a `warning` stroke
//! with its name in `text_primary`; a step that has not started is muted.

use lurq::{
  app::theme::{BorderSize, PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::time::Timestamp;
use orchester_workspace::tasks::{
  detail::RunView,
  progress::{ProgressState, progress},
};

use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// A chip's height (`hDyRI`).
const CHIP_HEIGHT: f32 = 24.0;

/// The stages of `run`, the step with key `focus` drawn as the one the decision is about.
pub fn stages(run: &RunView, focus: Option<&str>, now: Timestamp) -> Element {
  let numbers = |key: &str| {
    run
      .steps
      .iter()
      .find(|step| step.key.as_str() == key)
      .map_or(0, |step| step.number)
  };
  Row::new()
    .id("task-stages")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .wrap()
    .with_children(progress(run, false, now).into_iter().map(|row| {
      let focused = focus == Some(&*row.id);
      chip(&row.id, (numbers(&row.id), &row.step), row.state, focused)
    }))
    .into()
}

fn chip(key: &str, (number, name): (usize, &str), state: ProgressState, focused: bool) -> Element {
  let (glyph, color) = look(state, focused);
  let (stroke, name_color) = if focused {
    (PaletteColor::Warning, PaletteColor::TextPrimary)
  } else if state == ProgressState::Waiting {
    (PaletteColor::Border, PaletteColor::TextMuted)
  } else {
    (PaletteColor::Border, PaletteColor::TextSecondary)
  };
  stage_chip(
    &format!("task-stage-{key}"),
    (Some(&format!("{number:02}")), name),
    ((glyph, color), name_color, stroke),
  )
  .describe("focused", if focused { "true" } else { "false" })
  .into()
}

/// A chip: its glyph in its color, its number when it has one, and its name, in a pill of `stroke`.
pub fn stage_chip(
  id: &str,
  (number, name): (Option<&str>, &str),
  ((glyph, color), name_color, stroke): ((Icon, PaletteColor), PaletteColor, PaletteColor),
) -> Row {
  Row::new()
    .id(id.to_owned())
    .height(Dimension::Px(CHIP_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Inline.role()))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(ExtraRadius::Full.role())
    .child(icon(glyph, IconSize::Sm, color))
    .with_children(number.map(|number| Text::new(number).variant(ExtraTypography::MonoNote.role())))
    .child(
      Text::new(name)
        .variant(ExtraTypography::Note.role())
        .color(name_color)
        .nowrap(),
    )
}

/// The icon of a step in `state` and its color; the step the decision is about waits for the person.
fn look(state: ProgressState, focused: bool) -> (Icon, PaletteColor) {
  match state {
    ProgressState::StoppedAtLimit | ProgressState::Paused => (Icon::CirclePause, PaletteColor::Warning),
    _ if focused => (Icon::Hand, PaletteColor::Warning),
    ProgressState::Complete => (Icon::CircleCheck, PaletteColor::Success),
    ProgressState::NeedsYou => (Icon::Hand, PaletteColor::Warning),
    ProgressState::Working => (Icon::LoaderCircle, PaletteColor::TextSecondary),
    ProgressState::Failed(_) => (Icon::CircleX, PaletteColor::Danger),
    ProgressState::Interrupted | ProgressState::CantContinue => (Icon::CirclePause, PaletteColor::TextMuted),
    ProgressState::Skipped => (Icon::SkipForward, PaletteColor::TextMuted),
    ProgressState::Cancelled => (Icon::CircleSlash, PaletteColor::TextMuted),
    ProgressState::Waiting => (Icon::CircleDashed, PaletteColor::TextMuted),
  }
}
