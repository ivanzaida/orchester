//! Acceptance criteria of the new task form (`Sc7Ua`): one text area per criterion that wraps at
//! the column's width and grows with its lines, its marker and remove control beside the first
//! line. Enter adds a criterion after the one being typed and Backspace in an empty one removes it,
//! the focus moving along; neither puts a line break in a criterion.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::keyboard_event::KeyboardEvent,
    theme::{FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text, TextInput},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, Style, dimension::Dimension},
};
use orchester_core::task::MissingDetail;

use super::{FormState, NewTaskForm, missing_note};
use crate::{
  theme::{DISABLED_OPACITY, ExtraSpacing, ExtraTypography, input_line_height},
  ui::{
    field::locked_input,
    icon::{Icon, IconSize, icon},
    pressable::{Ring, keyboard_stop},
  },
};

/// The lines a criterion grows between before it scrolls.
const CRITERION_ROWS: (usize, usize) = (1, 10);

/// One acceptance criterion row. `id` keeps the row's input when rows above it are removed.
#[derive(Clone)]
pub struct Criterion {
  id: u64,
  pub text: Signal<String>,
  hovered: Signal<bool>,
  remove_focused: Signal<bool>,
  input: ElementRef,
  /// The keyboard added this row, or removed the one after it: its input takes the focus.
  takes_focus: Signal<bool>,
}

impl lurq::app::component::DevtoolsInspectable for Criterion {}

impl Criterion {
  pub fn new(id: u64) -> Self {
    Self {
      id,
      text: Signal::new(String::new()),
      hovered: Signal::new(false),
      remove_focused: Signal::new(false),
      input: ElementRef::new(),
      takes_focus: Signal::new(false),
    }
  }

  /// A row holding a saved criterion.
  pub fn with_text(id: u64, text: &str) -> Self {
    let criterion = Self::new(id);
    criterion.text.set(text.to_owned());
    criterion
  }
}

/// The id of a row added to `rows`.
fn next_id(rows: &[Criterion]) -> u64 {
  rows.iter().map(|row| row.id + 1).max().unwrap_or_default()
}

/// Acceptance criteria (`Sc7Ua`): one input per criterion, rows can be added and removed.
pub(super) fn criteria_field(ctx: &Ctx, form: &NewTaskForm, state: &FormState) -> Element {
  let criteria = form.criteria.get();
  let locked = state.queued || state.starting;
  let removable = criteria.len() > 1 && !locked;
  let rows: Vec<Element> = criteria
    .iter()
    .map(|criterion| criterion_row(ctx, &form.criteria, criterion, removable, locked))
    .collect();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(Text::new(&ctx.t("tasks.new.criteria")).variant(ExtraTypography::FormLabel.role()))
    .with_children(rows)
    .child(add_criterion(ctx, &form.criteria, locked))
    .with_children(missing_note(ctx, state, MissingDetail::AcceptanceCriteria))
    // Add criterion may be last; its focus ring reaches past the column.
    .overflow_visible()
    .into()
}

/// *+ Add criterion* (`Sc8e9`). The locked form keeps it in its place (`fblW6` in `WOdDe`,
/// `T6bVyU`), unavailable like the form's other controls that cannot be used there.
fn add_criterion(ctx: &Ctx, criteria: &Signal<Vec<Criterion>>, locked: bool) -> Element {
  // The link's padding keeps its focus ring off the text, and an equal shift back keeps the text
  // in line with the criterion markers (`Sc8e9` in `l2YA4`). The ring reaches into the room the
  // form's scroll content keeps on its left.
  let padding = ctx.theme().spacing_value(SpacingSize::Xs).resolve(0.0);
  let label = Text::new(&ctx.t("tasks.new.addCriterion"))
    .variant(TypographyStyle::Caption)
    .color(PaletteColor::TextPrimary);
  let add = |button: Button| {
    button
      .id("add-criterion")
      .padding_horizontal(SpacingSize::Xs)
      .offset(-padding, 0.0)
      .rounded(RadiusSize::Sm)
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(label)
  };
  if locked {
    return add(Button::empty().button())
      .describe("state", "unavailable")
      .focusable(false)
      .cursor(CursorIcon::NotAllowed)
      .opacity(DISABLED_OPACITY)
      .into();
  }
  let criteria = criteria.clone();
  add(keyboard_stop(Button::empty().button(), Ring::Outer))
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .on_click(move |_| {
      criteria.update(|criteria| {
        let next = next_id(criteria);
        criteria.push(Criterion::new(next));
      });
    })
    .into()
}

fn criterion_row(
  ctx: &Ctx,
  criteria: &Signal<Vec<Criterion>>,
  criterion: &Criterion,
  removable: bool,
  locked: bool,
) -> Element {
  // A criterion reads in `$muted` (`Y3PwR0` in `WOdDe`, `egNyW` in `dRN4m`); the placeholder of an
  // empty one stays apart from it in `$subtle`.
  let theme = ctx.theme();
  let text = |color| {
    FormTextRole {
      typography: TypographyStyle::FieldLabel,
      color,
    }
    .resolve(&theme.typography(), &theme.palette())
  };
  let style = text(PaletteColor::TextSecondary);
  // A text area is as tall as its lines; the marker and remove control center on the first.
  let first_line = input_line_height(&style);
  if criterion.takes_focus.get_untracked() {
    criterion.takes_focus.set(false);
    ctx.focus(&criterion.input);
  }
  let input = TextInput::styled(criterion.text.clone(), style)
    .placeholder_style(text(PaletteColor::TextMuted))
    .id(Arc::<str>::from(format!("criterion-{}", criterion.id)))
    .textarea()
    .rows(CRITERION_ROWS.0, CRITERION_ROWS.1)
    .flex(1.0)
    .caret_color(PaletteColor::TextPrimary)
    .placeholder(&ctx.t("tasks.new.criterionPlaceholder"))
    .ref_element(criterion.input.clone());
  let input = if locked {
    locked_input(input, true)
  } else {
    input.on_key_down(criterion_keys(criteria, criterion))
  };
  let row = Row::new()
    .key(Arc::<str>::from(format!("criterion-{}", criterion.id)))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Start)
    .on_mouse_enter({
      let hovered = criterion.hovered.clone();
      move || hovered.set(true)
    })
    .on_mouse_leave({
      let hovered = criterion.hovered.clone();
      move || hovered.set(false)
    })
    .child(beside_first_line(
      first_line,
      icon(Icon::Circle, IconSize::Marker, PaletteColor::TextMuted),
    ))
    .child(input);
  if !removable {
    return row.into();
  }
  row
    .child(beside_first_line(first_line, remove_button(criteria, criterion)))
    .into()
}

/// `control` centered on a criterion's first line, which is `height` tall.
fn beside_first_line(height: f32, control: impl Into<Element>) -> Row {
  Row::new()
    .height(Dimension::Px(height))
    .align_items(Alignment::Center)
    .overflow_visible()
    .child(control)
}

/// The criterion's ×, shown while the pointer is on its row or it has the focus.
fn remove_button(criteria: &Signal<Vec<Criterion>>, criterion: &Criterion) -> Button {
  let (criteria, id) = (criteria.clone(), criterion.id);
  let shown = criterion.hovered.get() || criterion.remove_focused.get();
  let focused = criterion.remove_focused.clone();
  let blurred = criterion.remove_focused.clone();
  keyboard_stop(Button::empty().button(), Ring::Inset)
    .id(Arc::<str>::from(format!("remove-criterion-{id}")))
    .align_items(Alignment::Start)
    .opacity(if shown { 1.0 } else { 0.0 })
    .cursor(CursorIcon::Pointer)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .on_focus(move || focused.set(true))
    .on_blur(move || blurred.set(false))
    .on_click(move |_| criteria.update(|rows| rows.retain(|row| row.id != id)))
    .child(icon(Icon::X, IconSize::Dismiss, PaletteColor::TextSecondary))
}

/// Enter in the criterion adds an empty one after it; Backspace in it while it is empty removes it
/// when another is left, the focus going to the one before (the next one for the first). Keys reach
/// every row's handler, so only the row whose input has the focus acts.
fn criterion_keys(
  criteria: &Signal<Vec<Criterion>>,
  criterion: &Criterion,
) -> impl Fn(KeyboardEvent) + Send + Sync + use<> {
  let (criteria, id) = (criteria.clone(), criterion.id);
  let (input, text) = (criterion.input.clone(), criterion.text.clone());
  move |event: KeyboardEvent| {
    if !input.focused() {
      return;
    }
    match event.key.as_str() {
      "Enter" => {
        // Never a line break, also while an input method composes, whose Enter only confirms.
        event.prevent_default();
        if !event.composing {
          criteria.update(|rows| add_after(rows, id));
        }
      }
      "Backspace" if text.get_untracked().is_empty() && criteria.get_untracked().len() > 1 => {
        event.prevent_default();
        criteria.update(|rows| remove_empty(rows, id));
      }
      _ => {}
    }
  }
}

/// Adds an empty criterion after the one with `id`, to be typed in next.
fn add_after(rows: &mut Vec<Criterion>, id: u64) {
  let added = Criterion::new(next_id(rows));
  added.takes_focus.set(true);
  let at = rows.iter().position(|row| row.id == id).map_or(rows.len(), |at| at + 1);
  rows.insert(at, added);
}

/// Removes the criterion with `id`; the one before it, or else the one after it, takes the focus.
fn remove_empty(rows: &mut Vec<Criterion>, id: u64) {
  let Some(at) = rows.iter().position(|row| row.id == id) else {
    return;
  };
  rows.remove(at);
  if let Some(next) = rows.get(at.saturating_sub(1)) {
    next.takes_focus.set(true);
  }
}
