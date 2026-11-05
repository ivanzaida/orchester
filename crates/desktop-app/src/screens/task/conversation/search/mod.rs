//! Searching the transcript (ORC-306, `Task page · redesign` `C3wC8` 13 `p3864h` closed, 13b `c0CQp` with a query,
//! 13c `W60Jr6` no match, 13d `yxLHS` a match in a folded step and a tool group, 13e `z2ai2h` a match in a call's
//! output, 13f `aoVlE` its parts and rules). Ctrl F (⌘ F on macOS) or the search button at the right end of the tabs
//! opens the find bar over the chat ([`bar`]) with the focus in its field; pressed while it is open, it selects the
//! query again. The run is searched off the UI thread ([`run`]) 150 ms after typing stops, plain text with case and
//! accents ignored (`orchester_workspace::tasks::timeline::find`). The places found are counted and marked
//! ([`marks`]); Enter and Shift Enter, or the chevrons, go to the next and previous one round past the ends. The
//! current one is always shown: the step that folds it, its tool group, the call holding it open; the search marks
//! what it opened, and folds it again when it closes. Esc or the cross closes the bar: the marks go, and the chat
//! stays where the current place was. While the run goes on, new places add to the count and the current one stays
//! where it is. The query stays while the task page is open.
//!
//! A message fills behind its matched words, one fill per laid-out line, with its Markdown rendered as ever (ORC-622,
//! lurq's `TextHighlight`, `ui::chat::found`); a card, a problem or a notice, which says several texts, is marked on
//! its row instead ([`marks::row_mark`]).

mod bar;
pub(super) mod marks;
mod run;
pub(super) mod view;

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, events::KeyboardEvent},
  components::TextInputController,
  core::Signal,
};
use orchester_workspace::tasks::timeline::{
  find::{Hit, Spot},
  view::Row,
};

use super::timeline::Toggles;
use crate::navigation::shortcuts::Keys;

/// The rows an open tool group lists before *Show N more* (`super::timeline` chips).
const FIRST_ROWS: usize = 7;

/// The search of the task page's transcript: whether its bar is open, what it looks for and which place is current.
/// The task's frame keeps it, so the query stays as the tabs and the task's runs change.
#[derive(Clone)]
pub struct TranscriptSearch {
  pub(super) open: Signal<bool>,
  pub(super) query: Signal<String>,
  /// The current place, an index into what the search found.
  pub(super) at: Signal<usize>,
  /// The field takes the focus, its query selected, at the bar's next render.
  focus: Signal<bool>,
  pub(super) field: TextInputController,
}

impl TranscriptSearch {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      query: ctx.signal(String::new()),
      at: ctx.signal(0),
      focus: ctx.signal(false),
      field: TextInputController::new(),
    }
  }

  /// Whether the bar is open; reading it re-renders the reader as it opens and closes.
  pub fn is_open(&self) -> bool {
    self.open.get()
  }

  /// Opens the bar with the focus in its field and its query selected.
  pub fn show(&self) {
    if !self.open.get_untracked() {
      self.open.set(true);
    }
    self.focus.set(true);
  }

  pub(super) fn close(&self) {
    self.open.set(false);
  }

  /// Moves the current place to the next (or previous) of `count`, round past the ends.
  pub(super) fn step(&self, count: usize, forward: bool) {
    if count == 0 {
      return;
    }
    self.at.update(|at| {
      let now = (*at).min(count - 1);
      *at = if forward {
        (now + 1) % count
      } else {
        (now + count - 1) % count
      };
    });
  }

  /// The field takes the focus with its query selected, once it is mounted.
  pub(super) fn take_focus(&self, ctx: &Ctx) {
    if !self.focus.get() {
      return;
    }
    self.focus.set(false);
    self.field.focus(ctx);
    let length = self.query.get_untracked().len();
    // The field mounts with this render: until it is, there is nothing to select, and the focus alone is asked for.
    if self.field.select(0, length).is_err() {
      tracing::debug!("the search field is not mounted yet: its query is not selected");
    }
  }
}

/// What Ctrl F (⌘ F) does on the Conversation tab: opens the search, or selects its query again. Esc closes an open
/// search from anywhere in the transcript, but not from the composer, whose Esc is its own.
pub(super) fn keys(search: &TranscriptSearch) -> impl Fn(KeyboardEvent) + Send + Sync + 'static + use<> {
  let search = search.clone();
  move |event: KeyboardEvent| {
    let keys = Keys::of(&event);
    if keys.only_command() && keys.is("f", "KeyF") {
      search.show();
      event.prevent_default();
    } else if event.key == "Escape" && !event.text_input_focused && search.open.get_untracked() {
      search.close();
      event.prevent_default();
    }
  }
}

/// The rows to open so that `hit` shows, among those that hide it now: the step its fold folds, its tool group, the
/// call whose output or details it is in, and the group's rows past its first seven. `rows` are the chat's rows.
pub(super) fn to_open(hit: &Hit, rows: &[Arc<Row>], toggles: &Toggles) -> Vec<Arc<str>> {
  let mut keys = Vec::new();
  if let Some(step) = &hit.step
    && rows
      .iter()
      .any(|row| matches!(&**row, Row::Folded { steps, .. } if steps.iter().any(|(key, _)| key == step)))
  {
    keys.push(step.clone());
  }
  let Some(call) = &hit.call else {
    if let Some(row) = rows.iter().find(|row| holds(row, &hit.item))
      && let Row::Approvals { key, open: false, .. } = &**row
    {
      keys.push(key.clone());
    }
    return keys;
  };
  if !toggles.is_open(&hit.item, false) {
    keys.push(hit.item.clone());
  }
  let all: Arc<str> = format!("{}-all", hit.item).into();
  if call.rank >= FIRST_ROWS && !toggles.is_open(&all, false) {
    keys.push(all);
  }
  if matches!(hit.spot, Spot::Detail | Spot::Output(_)) && !toggles.is_open(&call.key, false) {
    keys.push(call.key.clone());
  }
  keys
}

/// The key of the chat's row that shows the item `item`, among `rows`.
pub(super) fn row_of(rows: &[Arc<Row>], item: &str) -> Option<Arc<str>> {
  rows.iter().find(|row| holds(row, item)).map(|row| row.key().clone())
}

/// Whether `row` shows the item `item`: its own, an agent's tool group, a request among Autopilot's.
fn holds(row: &Row, item: &str) -> bool {
  match row {
    Row::Agent(agent) => &*agent.key == item || agent.chips.iter().any(|chip| &*chip.key == item),
    Row::Approvals { items, .. } => items.iter().any(|approved| &*approved.key == item),
    other => &**other.key() == item,
  }
}
