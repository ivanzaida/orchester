//! Acceptance criteria inline (`Criteria` `rxCYt`): a head with the count and what reviewers do with them (12/500
//! `text_secondary` and 12 `text_muted`), then one row per criterion (`Criterion 1` `cScHP`): its number in Geist Mono
//! 11 `text_muted` and its text in 13, 12 apart, 8 over and under, a `border` line over it, typed in place, its × at
//! the end under the pointer or while it has the focus; *+ Add criterion* last, a `border` line under it too
//! (`Add criterion` `iqH3b`, `lmfff`). The keyboard
//! edits the list as the deprecated desktop's form does, and moves rows too: Enter adds a criterion after the one typed
//! in, so Enter in the last adds the next; Backspace in an empty one removes it, Alt+↑ and Alt+↓ move it; the focus
//! follows. None of them puts a line break in a criterion.

use lurq::{
  app::{
    ctx::Ctx,
    events::{TextInputEvent, keyboard_event::KeyboardEvent},
    theme::{BorderSize, FormTextRole, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextInput},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::state::View;
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, TaskFormText},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
    pressable::{Focus, pressable},
  },
};

/// A criterion number's column (`Num` `wHKCE`).
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

/// A row's frame: 12 between its parts, 8 over and under them, a `border` line over it.
fn row_frame() -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
}

pub fn criteria(ctx: &mut Ctx, view: &View, criteria: &Signal<Vec<Criterion>>) -> Element {
  let rows = criteria.get();
  let locked = view.locked();
  let items: Vec<Element> = rows
    .iter()
    .enumerate()
    .map(|(index, row)| criterion_row(ctx, criteria, row, index, locked))
    .collect();
  Column::new()
    .id("task-form-criteria")
    .width(Dimension::Pct(100.0))
    .child(head(ctx, rows.len()))
    .with_children(items)
    .with_children((!locked).then(|| add_row(ctx, criteria, rows.is_empty())))
    .into()
}

/// *Acceptance criteria* with how many there are and what reviewers do with them (`Head` `c8Z6C6`).
fn head(ctx: &Ctx, count: usize) -> Row {
  let muted = |text: &str| {
    Text::new(text)
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap()
  };
  Row::new()
    .id("task-form-criteria-head")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().bottom(ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("task_form.fields.criteria.label"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .with_children((count > 0).then(|| muted(&count.to_string()).id("task-form-criteria-count")))
    .child(muted(&ctx.t("task_form.fields.criteria.hint")).id("task-form-criteria-hint"))
}

/// *+ Add criterion* (`Add criterion` `iqH3b`): a criterion at the end, to type in next; once there are criteria it
/// says that Enter in the last one adds the next.
fn add_row(ctx: &Ctx, criteria: &Signal<Vec<Criterion>>, first: bool) -> Element {
  let criteria = criteria.clone();
  let label = ctx.t(if first {
    "task_form.fields.criteria.add"
  } else {
    "task_form.fields.criteria.addNext"
  });
  let press = pressable(move || {
    criteria.update(|rows| {
      let added = Criterion::new(next_id(rows), "");
      added.takes_focus.set(true);
      rows.push(added);
    });
  });
  // The list's last line closes it with a second `border` line (`lmfff`).
  let frame = row_frame().border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
  Focus::Keyboard
    .apply(press)
    .id("task-form-add-criterion")
    .describe("a11y_name", ctx.t("task_form.fields.criteria.add"))
    .width(Dimension::Pct(100.0))
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(
      frame
        .child(icon(Icon::Plus, IconSize::Base, PaletteColor::TextMuted))
        .child(
          Text::new(&label)
            .variant(TaskFormText::Criterion.role())
            .color(PaletteColor::TextMuted),
        ),
    )
    .into()
}

fn criterion_row(
  ctx: &mut Ctx,
  criteria: &Signal<Vec<Criterion>>,
  row: &Criterion,
  index: usize,
  locked: bool,
) -> Element {
  if row.takes_focus.get_untracked() {
    row.takes_focus.set(false);
    ctx.focus(&row.input);
  }
  let number = (index + 1).to_string();
  let frame = row_frame()
    .id(format!("task-form-criterion-{number}"))
    .key(format!("criterion-{}", row.id))
    .on_mouse_enter({
      let hovered = row.hovered.clone();
      move || hovered.set(true)
    })
    .on_mouse_leave({
      let hovered = row.hovered.clone();
      move || hovered.set(false)
    })
    .child(
      Text::new(&format!("{:02}", index + 1))
        .width(Dimension::Px(NUMBER_WIDTH))
        .variant(ExtraTypography::MonoNote.role()),
    )
    .child(criterion_input(ctx, criteria, row, &number, locked));
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
      typography: TaskFormText::Criterion.role(),
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
      .on_input(|event: TextInputEvent| event.prevent_default())
      .focusable(false);
  }
  input.on_key_down(keys(criteria, row))
}

/// The criterion's ×, seen while the pointer is on its row or it or its input has the focus. It takes no height of
/// the row, which stays 8 over and under one line (`cScHP`): it hangs centred on the row's middle.
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
    .height(Dimension::Px(0.0))
    .align_items(Alignment::Center)
    .overflow_visible()
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
      // Ctrl+Enter and Cmd+Enter are Create & run's (`super::keys`).
      "Enter" if event.ctrl || event.meta => return,
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
