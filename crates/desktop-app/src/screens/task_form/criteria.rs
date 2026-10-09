//! Acceptance criteria (`Row · Acceptance criteria` `KFAGh`, `Criterion row` `fjXep`): one numbered row
//! per criterion in a box, its × at the end under the pointer or while it has the focus, and *Add
//! criterion* under them; without criteria only the row's description and *Add criterion* show
//! (`s7jKkA`). The keyboard edits the list as the deprecated desktop's form does, and moves rows too:
//! Enter adds a criterion after the one typed in, Backspace in an empty one removes it, Alt+↑ and Alt+↓
//! move it; the focus follows. None of them puts a line break in a criterion.

use lurq::{
  app::{
    ctx::Ctx,
    events::keyboard_event::KeyboardEvent,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextInput},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  sections::{Label, Place, stacked_row},
  state::View,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    icon_button::IconButton,
  },
};

/// A criterion row's least height (`Criterion 1` `ebWkj`) and its number's column (`Number` `SLzmn`).
const ROW_HEIGHT: f32 = 34.0;
const NUMBER_WIDTH: f32 = 14.0;
/// The lines a criterion grows to before its lines scroll.
const MAX_LINES: usize = 6;

/// One criterion row. `id` keeps the row's input when rows around it are added, removed or moved.
#[derive(Clone)]
pub struct Criterion {
  id: u64,
  pub text: Signal<String>,
  hovered: Signal<bool>,
  focused: Signal<bool>,
  input: ElementRef,
  /// The keyboard added or moved this row, or removed the one after it: its input takes the focus.
  takes_focus: Signal<bool>,
}

impl lurq::app::component::DevtoolsInspectable for Criterion {}

impl Criterion {
  fn new(id: u64, text: &str) -> Self {
    Self {
      id,
      text: Signal::new(text.to_owned()),
      hovered: Signal::new(false),
      focused: Signal::new(false),
      input: ElementRef::new(),
      takes_focus: Signal::new(false),
    }
  }

  /// Rows holding `texts`, in order.
  pub fn rows(texts: &[String]) -> Vec<Self> {
    (0..).zip(texts).map(|(id, text)| Self::new(id, text)).collect()
  }
}

/// The id of a row added to `rows`.
fn next_id(rows: &[Criterion]) -> u64 {
  rows.iter().map(|row| row.id + 1).max().unwrap_or_default()
}

pub fn criteria_row(ctx: &mut Ctx, view: &View, criteria: &Signal<Vec<Criterion>>) -> Element {
  let rows = criteria.get();
  let locked = view.locked();
  let mut label = Label::new("task-form-criteria", ctx.t("task_form.fields.criteria.label"));
  if rows.is_empty() {
    label = label.description(ctx.t("task_form.fields.criteria.description"));
  }
  let list = (!rows.is_empty()).then(|| {
    let count = rows.len();
    let items: Vec<Element> = rows
      .iter()
      .enumerate()
      .map(|(index, row)| criterion_row(ctx, criteria, row, (index, count), locked))
      .collect();
    Column::new()
      .id("task-form-criteria-list")
      .width(Dimension::Pct(100.0))
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .clip()
      .with_children(items)
  });
  let control = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .with_children(list)
    .with_children((!locked).then(|| add_button(ctx, criteria)));
  stacked_row(label, control, Place::Last)
}

/// *+ Add criterion* (`Add criterion` `pYCrW`): a criterion at the end, to type in next.
fn add_button(ctx: &Ctx, criteria: &Signal<Vec<Criterion>>) -> Element {
  let criteria = criteria.clone();
  ActionButton::new(
    "task-form-add-criterion",
    ButtonKind::Ghost,
    ctx.t("task_form.fields.criteria.add"),
  )
  .icon(Icon::Plus)
  .in_form()
  .on_click(move || {
    criteria.update(|rows| {
      let added = Criterion::new(next_id(rows), "");
      added.takes_focus.set(true);
      rows.push(added);
    });
  })
  .into()
}

fn criterion_row(
  ctx: &mut Ctx,
  criteria: &Signal<Vec<Criterion>>,
  row: &Criterion,
  (index, count): (usize, usize),
  locked: bool,
) -> Element {
  if row.takes_focus.get_untracked() {
    row.takes_focus.set(false);
    ctx.focus(&row.input);
  }
  let number = (index + 1).to_string();
  let frame = Row::new()
    .id(format!("task-form-criterion-{}", index + 1))
    .key(format!("criterion-{}", row.id))
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(ROW_HEIGHT))
    .padding(
      Padding::new()
        .left(SpacingSize::Md)
        .right(ExtraSpacing::Snug.role())
        .top(SpacingSize::Xs)
        .bottom(SpacingSize::Xs),
    )
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .on_mouse_enter({
      let hovered = row.hovered.clone();
      move || hovered.set(true)
    })
    .on_mouse_leave({
      let hovered = row.hovered.clone();
      move || hovered.set(false)
    })
    .child(
      Text::new(&number)
        .width(Dimension::Px(NUMBER_WIDTH))
        .variant(ExtraTypography::MonoNote.role()),
    )
    .child(criterion_input(ctx, criteria, row, &number, locked));
  // The criterion being typed in takes the focused stroke (`Criterion 4` in `v2c76`); the others are
  // divided by a line.
  let frame = if row.focused.get() && !locked {
    frame.border(Border::inside(BorderSize::Sm, PaletteColor::BorderFocus))
  } else if index + 1 < count {
    frame.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
  } else {
    frame
  };
  if locked {
    return frame.into();
  }
  frame.child(remove_button(ctx, criteria, row, &number)).into()
}

fn criterion_input(
  ctx: &Ctx,
  criteria: &Signal<Vec<Criterion>>,
  row: &Criterion,
  number: &str,
  locked: bool,
) -> TextInput {
  let theme = ctx.theme();
  let text = |color| {
    FormTextRole {
      typography: TypographyStyle::Description,
      color,
    }
    .resolve(&theme.typography(), &theme.palette())
  };
  let color = if locked {
    PaletteColor::TextSecondary
  } else {
    PaletteColor::TextPrimary
  };
  let (gained, lost) = (row.focused.clone(), row.focused.clone());
  let input = TextInput::styled(row.text.clone(), text(color))
    .placeholder_style(text(PaletteColor::TextMuted))
    .placeholder(&ctx.t("task_form.fields.criteria.placeholder"))
    .id(format!("task-form-criterion-{number}-input"))
    .describe(
      "a11y_name",
      ctx.t_args("task_form.fields.criteria.name", [("number", number)]),
    )
    .multiline()
    .rows(1, MAX_LINES)
    .flex(1.0)
    .caret_color(PaletteColor::TextPrimary)
    .ref_element(row.input.clone())
    .on_focus(move || gained.set(true))
    .on_blur(move || lost.set(false));
  if locked {
    return input
      .on_input(|event: lurq::app::events::TextInputEvent| event.prevent_default())
      .focusable(false);
  }
  input.on_key_down(keys(criteria, row))
}

/// The criterion's ×, seen while the pointer is on its row or it or its input has the focus.
fn remove_button(ctx: &mut Ctx, criteria: &Signal<Vec<Criterion>>, row: &Criterion, number: &str) -> Element {
  let shown = row.hovered.get() || row.focused.get();
  let (criteria, id) = (criteria.clone(), row.id);
  let button = IconButton::new(
    format!("task-form-criterion-{number}-remove"),
    Icon::X,
    ctx.t_args("task_form.fields.criteria.remove", [("number", number)]),
  )
  .small()
  .in_form()
  .on_click(move || criteria.update(|rows| rows.retain(|row| row.id != id)))
  .render(ctx);
  Row::new()
    .opacity(if shown { 1.0 } else { 0.0 })
    .describe("shown", if shown { "true" } else { "false" })
    .child(button)
    .into()
}

/// Enter, Backspace in an empty criterion, Alt+↑ and Alt+↓. Keys reach every row's handler, so only the
/// row whose input has the focus acts.
fn keys(criteria: &Signal<Vec<Criterion>>, row: &Criterion) -> impl Fn(KeyboardEvent) + Send + Sync + use<> {
  let (criteria, id) = (criteria.clone(), row.id);
  let (input, text) = (row.input.clone(), row.text.clone());
  move |event: KeyboardEvent| {
    if !input.focused() {
      return;
    }
    let edit = match event.key.as_str() {
      "Enter" => {
        // Never a line break, also while an input method composes, whose Enter only confirms.
        event.prevent_default();
        if event.composing {
          return;
        }
        Edit::AddAfter
      }
      "Backspace" if text.get_untracked().is_empty() => Edit::Remove,
      "ArrowUp" if event.alt => Edit::MoveUp,
      "ArrowDown" if event.alt => Edit::MoveDown,
      _ => return,
    };
    event.prevent_default();
    criteria.update(|rows| match edit {
      Edit::AddAfter => add_after(rows, id),
      Edit::Remove => remove_empty(rows, id),
      Edit::MoveUp => move_up(rows, id),
      Edit::MoveDown => move_down(rows, id),
    });
  }
}

/// What a key does to the list.
#[derive(Clone, Copy)]
enum Edit {
  AddAfter,
  Remove,
  MoveUp,
  MoveDown,
}

fn add_after(rows: &mut Vec<Criterion>, id: u64) {
  let added = Criterion::new(next_id(rows), "");
  added.takes_focus.set(true);
  let at = rows.iter().position(|row| row.id == id).map_or(rows.len(), |at| at + 1);
  rows.insert(at, added);
}

/// Removes the empty criterion `id`; the one before it, or else the one after it, takes the focus.
fn remove_empty(rows: &mut Vec<Criterion>, id: u64) {
  let Some(at) = rows.iter().position(|row| row.id == id) else {
    return;
  };
  rows.remove(at);
  if let Some(next) = rows.get(at.saturating_sub(1)) {
    next.takes_focus.set(true);
  }
}

fn move_up(rows: &mut [Criterion], id: u64) {
  if let Some(at) = rows.iter().position(|row| row.id == id).filter(|at| *at > 0) {
    rows.swap(at, at - 1);
    rows[at - 1].takes_focus.set(true);
  }
}

fn move_down(rows: &mut [Criterion], id: u64) {
  if let Some(at) = rows
    .iter()
    .position(|row| row.id == id)
    .filter(|at| at + 1 < rows.len())
  {
    rows.swap(at, at + 1);
    rows[at + 1].takes_focus.set(true);
  }
}

/// The texts of `rows`, in order.
pub fn texts(rows: &[Criterion]) -> Vec<String> {
  rows.iter().map(|row| row.text.get()).collect()
}
