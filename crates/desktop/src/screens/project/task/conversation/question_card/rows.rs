//! A question's rows on its card (C·15 `lS9BF`): each option as a row (radius 7, padding 8 / 10) with its
//! control (a 16 radio, or a 14 checkbox with several allowed), its letter in an 18 key, its label and its
//! description under it; a picked row is `$hover` with its control filled and its key stroked `$muted`. The
//! own answer's row comes last: its control and letter, then its field. Rows are 2 apart.

use lurq::{
  app::{
    ctx::Ctx,
    events::TextInputEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::question::{Question, QuestionOption};

use super::{
  CardState,
  keys::{RowAt, Spot},
};
use crate::{
  tasks::question::{Picks, letter},
  theme::{ExtraRadius, ExtraShadow, ExtraSpacing},
  ui::{
    field::text_input,
    icon::{Icon, IconSize, icon},
    pressable::TAB_STOP,
  },
};

/// The letter key's box (`lS9BF`).
const KEY_SIZE: f32 = 18.0;
/// The header chip's height (`lS9BF`).
const CHIP_HEIGHT: f32 = 20.0;

/// The question's header as a chip beside its title (*Docs layout*): 11 `$muted` on a `$border` stroke.
pub(super) fn header_chip(header: &str) -> Element {
  Row::new()
    .id("conversation-question-header")
    .height(Dimension::Px(CHIP_HEIGHT))
    .padding_horizontal(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .flex_shrink(0.0)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Chip.role())
    .child(Text::new(header).variant(TypographyStyle::Caption).nowrap())
    .into()
}

/// Question `index`'s rows: its options, then its own answer.
pub(super) fn options(ctx: &Ctx, state: &CardState, (index, question): (usize, &Question)) -> Element {
  let draft = state.draft.get();
  let picks = draft.of(index).cloned().unwrap_or_default();
  let ringed = state.keyed.get().then(|| state.focused.get()).flatten();
  let rows = question.options().iter().enumerate().map(|(option, shown)| {
    let spot = Spot {
      question: index,
      row: RowAt::Option(option),
    };
    option_row(
      state,
      (spot, question.multi_select()),
      (shown, picks.picked(option)),
      ringed == Some(spot),
    )
  });
  let own = Spot {
    question: index,
    row: RowAt::Own,
  };
  Column::new()
    .id(format!("conversation-question-options-{index}"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Hair.role())
    .with_children(rows)
    .child(own_row(ctx, state, (own, question), &picks))
    .into()
}

/// The control of a row: a radio for one answer, a checkbox for several.
fn control(several: bool, picked: bool) -> Element {
  let (glyph, size) = match (several, picked) {
    (false, false) => (Icon::Circle, IconSize::Option),
    (false, true) => (Icon::CircleDot, IconSize::Option),
    (true, false) => (Icon::Square, IconSize::Event),
    (true, true) => (Icon::SquareCheck, IconSize::Event),
  };
  let color = if picked {
    PaletteColor::Accent
  } else {
    PaletteColor::TextSecondary
  };
  icon(glyph, size, color).into()
}

/// The row's letter in its key: stroked `$muted` with a `$text` letter while picked.
pub(super) fn key(index: usize, picked: bool) -> Element {
  let (stroke, color) = if picked {
    (PaletteColor::TextSecondary, PaletteColor::TextPrimary)
  } else {
    (PaletteColor::Border, PaletteColor::TextSecondary)
  };
  Row::new()
    .size(Dimension::Px(KEY_SIZE), Dimension::Px(KEY_SIZE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(ExtraRadius::Key.role())
    .child(
      Text::new(&letter(index).map_or_else(String::new, String::from))
        .variant(TypographyStyle::Mono)
        .color(color)
        .nowrap(),
    )
    .into()
}

/// A row as a keyboard stop of its own: the ring only when the keyboard put the focus there.
fn row_button(state: &CardState, spot: Spot, (picked, ringed): (bool, bool)) -> Button {
  let (focused, keyed) = (state.focused.clone(), state.keyed.clone());
  let blurred = state.focused.clone();
  let button = Button::empty()
    .button()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .padding_horizontal(ExtraSpacing::Inline.role())
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Start)
    .rounded(RadiusSize::Md)
    .focusable(true)
    .tab_index(TAB_STOP)
    .cursor(CursorIcon::Pointer)
    .describe("picked", picked.to_string())
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .on_focus(move || focused.set(Some(spot)))
    .on_blur(move || {
      if blurred.get_untracked() == Some(spot) {
        blurred.set(None);
      }
    })
    .on_mouse_down(move |_| {
      if keyed.get_untracked() {
        keyed.set(false);
      }
    });
  let button = if picked {
    button.background(PaletteColor::AccentMuted)
  } else {
    button
  };
  if ringed {
    button.box_shadow(ExtraShadow::FocusRingInset.role())
  } else {
    button
  }
}

fn option_row(
  state: &CardState,
  (spot, several): (Spot, bool),
  (option, picked): (&QuestionOption, bool),
  ringed: bool,
) -> Element {
  let RowAt::Option(index) = spot.row else {
    return Column::new().into();
  };
  let target = state
    .fields
    .lock()
    .rows
    .get(spot.question)
    .and_then(|rows| rows.get(index))
    .cloned();
  let (draft, request) = (state.draft.clone(), state.request.clone());
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .child(
      Text::new(option.label())
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextPrimary),
    )
    .with_children(
      option
        .description()
        .map(|description| Text::new(description).variant(TypographyStyle::Description)),
    );
  let button = row_button(state, spot, (picked, ringed))
    .id(format!("conversation-question-option-{}-{index}", spot.question))
    .child(control(several, picked))
    .child(key(index, picked))
    .child(text)
    .on_click(move |_| draft.update(|draft| draft.pick(request.questions(), (spot.question, index))));
  match target {
    Some(target) => button.ref_element(target).into(),
    None => button.into(),
  }
}

/// The own answer's row: its control and letter, then its field, which takes the focus itself (its focused
/// stroke marks it); typing picks the row.
fn own_row(ctx: &Ctx, state: &CardState, (spot, question): (Spot, &Question), picks: &Picks) -> Element {
  let fields = state.fields.lock();
  let (Some(text), Some(input)) = (
    fields.texts.get(spot.question).cloned(),
    fields.inputs.get(spot.question).cloned(),
  ) else {
    return Column::new().into();
  };
  drop(fields);
  let (draft, request) = (state.draft.clone(), state.request.clone());
  let typed = move |event: TextInputEvent| {
    let value = event.new_value;
    draft.update(|draft| draft.type_own(request.questions(), spot.question, &value));
  };
  let (focused, blurred) = (state.focused.clone(), state.focused.clone());
  let field = text_input(ctx, "conversation-question-own", text, false, TypographyStyle::Body)
    .id(format!("conversation-question-own-{}", spot.question))
    .placeholder(&ctx.t("runs.conversation.question.ownPlaceholder"))
    .flex(1.0)
    .tab_index(TAB_STOP)
    .ref_element(input)
    .on_input(typed)
    .on_focus(move || focused.set(Some(spot)))
    .on_blur(move || {
      if blurred.get_untracked() == Some(spot) {
        blurred.set(None);
      }
    });
  let picked = picks.own_picked();
  let row = Row::new()
    .id(format!("conversation-question-own-row-{}", spot.question))
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .padding_horizontal(ExtraSpacing::Inline.role())
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .describe("picked", picked.to_string())
    .child(control(question.multi_select(), picked))
    .child(key(question.options().len(), picked))
    .child(field);
  if picked {
    row.background(PaletteColor::AccentMuted).into()
  } else {
    row.into()
  }
}
