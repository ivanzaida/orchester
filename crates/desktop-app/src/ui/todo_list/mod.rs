//! The agent's own todo list as a chat shows it (`Live todo list · redesign` `Ucmyu`, `docs/design/live-todo-list.md`):
//! while its step works, or while the Orchestrator answers, the box pinned over the composer ([`pinned`]); once that
//! ends, a chip (*Plan · 7 of 7*) on the step's divider or the answer's name line, which opens the list as it ended
//! under it, read only ([`chip`], [`ended`]). The items are the agent's, as its latest update left them: done ones
//! muted with a green check, the one in progress with the loader on a `hover` row, the rest waiting with an empty
//! circle (09 a `n1GXO`). No list shows nothing: the callers show these only for a list.

mod pinned;

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
use orchester_core::connection::stream::{TodoItem, TodoList, TodoState};

pub use self::pinned::{PinnedPlan, PinnedProps};
use super::{
  icon::{Icon, icon},
  pressable::pressable,
};
use crate::theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, TodoText};

/// Up to this many items the pinned box shows whole; past it, it folds its long list (07c `UfhkY`).
pub const WHOLE: usize = 7;
/// How many waiting items a folded long list shows after the one in progress.
pub const NEXT: usize = 3;

/// A row of the pinned box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
  /// The list's item at this index.
  Item(usize),
  /// This many done items, folded into one line with *Show* (`RRuHD`).
  Done(usize),
  /// This many more items to do, folded into one line with *Show all* (`COBz2`).
  More(usize),
}

/// The rows the pinned box shows of `items`: each item while there are at most [`WHOLE`] or the person `opened` the
/// list; past that, the done items as one line, the items in progress, the next [`NEXT`] waiting ones and how many
/// more wait.
pub fn shown(items: &[TodoItem], opened: bool) -> Vec<Shown> {
  if opened || items.len() <= WHOLE {
    return (0..items.len()).map(Shown::Item).collect();
  }
  let indices = |state: TodoState| {
    items
      .iter()
      .enumerate()
      .filter(move |(_, item)| item.state == state)
      .map(|(index, _)| index)
  };
  let done = indices(TodoState::Done).count();
  let waiting: Vec<usize> = indices(TodoState::Pending).collect();
  let more = waiting.len().saturating_sub(NEXT);
  (done > 0)
    .then_some(Shown::Done(done))
    .into_iter()
    .chain(indices(TodoState::InProgress).map(Shown::Item))
    .chain(waiting.into_iter().take(NEXT).map(Shown::Item))
    .chain((more > 0).then_some(Shown::More(more)))
    .collect()
}

/// How many of `list`'s items are done, and how many it has.
pub fn progress(list: &TodoList) -> (usize, usize) {
  let items = list.items();
  let done = items.iter().filter(|item| item.state == TodoState::Done).count();
  (done, items.len())
}

/// How many of `list`'s items a step left undone when it stopped: one that failed, or stopped or was interrupted while
/// it worked (09 f `rj5q6`); none for a step that `passed`, or that left nothing undone.
pub fn left_undone(list: &TodoList, passed: bool) -> Option<usize> {
  let (done, all) = progress(list);
  (!passed && done < all).then_some(all - done)
}

/// The item a folded box names: the one in progress, else the next that waits; none once every item is done.
pub fn current(list: &TodoList) -> Option<&TodoItem> {
  let items = list.items();
  items
    .iter()
    .find(|item| item.state == TodoState::InProgress)
    .or_else(|| items.iter().find(|item| item.state == TodoState::Pending))
}

/// *3 of 7*: done of all.
fn count(ctx: &Ctx, list: &TodoList) -> Arc<str> {
  let (done, all) = progress(list);
  ctx.t_args(
    "conversation.plan.count",
    [("done", done.to_string().as_str()), ("total", all.to_string().as_str())],
  )
}

/// The name a test or a tool reads an item's state by.
const fn state_name(state: TodoState) -> &'static str {
  match state {
    TodoState::Pending => "pending",
    TodoState::InProgress => "in_progress",
    TodoState::Done => "done",
  }
}

/// One item (`FK8b8`, `kzL74`, `vTz5z`): its mark and words, 8 apart, 5 over and under them and 8 at their sides. In a
/// list that `ended`, an item still in progress was left undone and waits as the others do (09 f `rj5q6`).
fn item_row(id: String, item: &TodoItem, ended: bool) -> Element {
  let state = match item.state {
    TodoState::InProgress if ended => TodoState::Pending,
    state => state,
  };
  let (glyph, color) = match state {
    TodoState::Done => (Icon::CircleCheck, PaletteColor::Success),
    TodoState::InProgress => (Icon::LoaderCircle, PaletteColor::TextSecondary),
    TodoState::Pending => (Icon::Circle, PaletteColor::TextMuted),
  };
  let text = Text::new(&item.text)
    .id(format!("{id}-text"))
    .variant(if state == TodoState::InProgress {
      TodoText::Current.role()
    } else {
      TodoText::Item.role()
    })
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  let text = if state == TodoState::Done {
    text.color(PaletteColor::TextMuted)
  } else {
    text
  };
  let row = line(id, (glyph, color), text.into());
  let row = row.describe("state", state_name(state));
  if state == TodoState::InProgress {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row.into()
  }
}

/// A row of the list: `mark` and `content`.
fn line(id: String, (glyph, color): (Icon, PaletteColor), content: Element) -> Row {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Pair.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Base, color))
    .child(content)
}

/// A folded line: `words` in `text_muted` after `mark`, and at its end `action`, which opens the whole list.
fn folded_line(
  id: String,
  mark: (Icon, PaletteColor),
  words: &str,
  (action, open): (Arc<str>, Arc<dyn Fn() + Send + Sync>),
) -> Element {
  let content = Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .align_items(Alignment::Center)
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(TodoText::Item.role())
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      pressable(move || open())
        .id(format!("{id}-show"))
        .child(Text::new(&action).variant(TodoText::Action.role()).nowrap()),
    );
  line(id, mark, content.into()).into()
}

/// The chip a finished step's divider or an ended answer's name line carries (`Tag chip` `FK4CS` as `bvhZU`):
/// `list-checks`, *Plan · 7 of 7* and a chevron that points down while the list is folded, up while it is `open`
/// under the line; a press opens or folds it.
pub fn chip(
  ctx: &Ctx,
  id: &str,
  (list, open): (&TodoList, bool),
  toggle: impl Fn() + Send + Sync + 'static,
) -> Element {
  let (done, all) = progress(list);
  let words = ctx.t_args(
    "conversation.plan.chip",
    [("done", done.to_string().as_str()), ("total", all.to_string().as_str())],
  );
  pressable(toggle)
    .id(id.to_owned())
    .describe("open", open.to_string())
    .flex_shrink(0.0)
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Xs)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Full.role())
    .child(icon(Icon::ListChecks, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .id(format!("{id}-label"))
        .variant(TodoText::Chip.role())
        .nowrap(),
    )
    .child(icon(
      if open { Icon::ChevronUp } else { Icon::ChevronDown },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .into()
}

/// The list as it ended, read only, under the line its chip opened it from (09 e `latq2`): every item, 1 apart and 6
/// in from the line's start, and `note`, what the step left undone when it stopped (09 f `gt3Yh`).
pub fn ended(id: &str, list: &TodoList, note: Option<&str>) -> Element {
  let items = list
    .items()
    .iter()
    .enumerate()
    .map(|(index, item)| item_row(format!("{id}-item-{index}"), item, true));
  let note = note.map(|note| {
    Row::new()
      .padding(
        Padding::new()
          .top(SpacingSize::Xs)
          .left(SpacingSize::Sm)
          .right(SpacingSize::Sm),
      )
      .child(Text::new(note).id(format!("{id}-note")).variant(TodoText::Note.role()))
  });
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding_left(ExtraSpacing::Snug.role())
    .spacing(ExtraSpacing::Hair.role())
    .with_children(items)
    .with_children(note)
    .into()
}
