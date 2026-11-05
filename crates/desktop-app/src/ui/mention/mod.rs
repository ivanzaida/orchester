//! @-mentions in a composer (ORC-312, ORC-316; `Mentions · redesign` `z8cUk`, `docs/design/mentions.md`), shared by
//! the Orchestrator's and a task Conversation's: typing `@` where a word starts opens the menu of the project's tasks,
//! runs and repository files ([`menu`]), and typing filters it; ↑ ↓ move, Enter, Tab or a click inserts, Esc closes it
//! and leaves the text. A mention is plain text in the draft (`@KON-12`, `@KON-12#9`, ``@`src/lib.rs` ``,
//! `orchester_workspace::mentions`) that the field draws as a chip, and Backspace right after it removes it whole.
//! lurq draws a chip as the text it decorates, so a file's chip shows its whole path, not only its name.

mod menu;
mod rows;

use std::{ops::Range, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{InputToken, InputTokens, Row, Stack, Text, TextInputController},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};
use orchester_workspace::mentions::{Mention, Typed, ending_at, find, typed_at};
use parking_lot::Mutex;

pub use self::menu::{MenuProps, menu};
use crate::theme::{ExtraTypography, mention_chip};

/// What the menu shows now, as the keys read it: what a choice replaces and the mentions of its rows, in order.
#[derive(Clone, Debug)]
struct Shown {
  replaces: Range<usize>,
  rows: Vec<Mention>,
}

/// A composer's message field with mentions: its draft, the controller that reads and edits it where the caret is, and
/// the menu's state. Made once with the field's component, so the menu keeps its state while the draft changes.
#[derive(Clone)]
pub struct MentionField {
  draft: Signal<String>,
  /// The field has the focus: only then is the menu open and does it take keys.
  focused: Signal<bool>,
  controller: TextInputController,
  /// The row with the keyboard's focus.
  focus: Signal<usize>,
  /// The `@` whose menu Esc closed: it stays closed until that `@` types no more.
  closed: Signal<Option<usize>>,
  /// Set to put the focus back in the field after a choice.
  refocus: Signal<bool>,
  /// Where the `@` being typed starts on its line, at the top of the field: the menu's left edge.
  marker: ElementRef,
  shown: Arc<Mutex<Option<Shown>>>,
}

impl PartialEq for MentionField {
  fn eq(&self, other: &Self) -> bool {
    self.draft.id() == other.draft.id() && Arc::ptr_eq(&self.shown, &other.shown)
  }
}

impl MentionField {
  /// The field of `draft`, whose composer keeps whether it has the focus in `focused`.
  pub fn new(ctx: &mut Ctx, draft: Signal<String>, focused: Signal<bool>) -> Self {
    Self {
      draft,
      focused,
      controller: TextInputController::new(),
      focus: ctx.signal(0),
      closed: ctx.signal(None),
      refocus: ctx.signal(false),
      marker: ElementRef::new(),
      shown: Arc::default(),
    }
  }

  /// What binds the field to its input (`TextInput::controller`).
  pub fn controller(&self) -> TextInputController {
    self.controller.clone()
  }

  /// The field's element, which the composer focuses.
  pub fn element(&self) -> ElementRef {
    self.controller.element_ref()
  }

  /// The mentions of `text`, the field's value now, drawn as chips.
  pub fn tokens(text: &str) -> InputTokens {
    let chips = find(text)
      .into_iter()
      .map(|found| InputToken::new(found.range, mention_chip()))
      .collect();
    InputTokens::new(text, chips).expect("mentions are disjoint one-line ranges of the text they were found in")
  }

  /// `input`, the field drawn in `typography`, in `id`, with the marker the menu opens at: an unseen copy of the words
  /// before the `@` being typed on its line (its chips as the field draws them), at the top of the field, ends at the
  /// `@`. A line the field wraps is measured as one line. The `id` keeps the field (and its focus) when files are
  /// attached over it.
  pub fn with_marker(&self, id: &str, input: impl Into<Element>, typography: TypographyStyle) -> Element {
    let pieces: Vec<Element> = self.draft.with(|text| {
      self
        .typed(text)
        .map(|typed| before_on_line(text, typed.at))
        .unwrap_or_default()
        .into_iter()
        .map(|(range, chip)| piece(&text[range], chip, typography))
        .collect()
    });
    let copy = Row::new()
      .absolute_position(0.0, 0.0)
      .opacity(0.0)
      .hit_test(HitTestBehavior::None)
      .align_items(Alignment::Start)
      .with_children(pieces)
      .child(
        // A menu opens only at an anchor with a size.
        Row::new()
          .ref_element(self.marker.clone())
          .size(Dimension::Px(1.0), Dimension::Px(1.0))
          .flex_shrink(0.0),
      );
    Stack::new()
      .id(id)
      .width(Dimension::Pct(100.0))
      .child(input)
      .child(copy)
      .into()
  }

  /// Where the menu opens: the marker [`Self::with_marker`] draws.
  fn marker(&self) -> ElementRef {
    self.marker.clone()
  }

  /// What the caret types for the menu in `text`, the field's value now: nothing while text is selected.
  fn typed<'a>(&self, text: &'a str) -> Option<Typed<'a>> {
    let selection = self.controller.selection().ok()?;
    if selection.anchor.is_some_and(|anchor| anchor != selection.caret) {
      return None;
    }
    typed_at(text, selection.caret)
  }

  /// Takes `event` when the menu or a chip has a use for it, before the field's own keys: whether it did. Backspace
  /// right after a chip selects it, so the field's Backspace removes it whole.
  pub fn key(&self, event: &KeyboardEvent) -> bool {
    if event.composing || event.default_prevented() || !self.focused.get_untracked() {
      return false;
    }
    if event.key == "Backspace" {
      self.select_chip_before_caret();
      return false;
    }
    let Some(shown) = self.shown.lock().clone() else {
      return false;
    };
    let rows = shown.rows.len();
    match event.key.as_str() {
      "Escape" => self.close(&shown),
      "ArrowDown" | "ArrowUp" if rows > 0 => {
        let at = self.focus.get_untracked().min(rows - 1);
        let next = if event.key == "ArrowDown" {
          (at + 1) % rows
        } else {
          (at + rows - 1) % rows
        };
        self.focus.set(next);
      }
      "Enter" | "Tab" if rows > 0 && !event.shift && !event.ctrl && !event.meta => {
        let at = self.focus.get_untracked().min(rows - 1);
        self.insert(shown.replaces, &shown.rows[at]);
      }
      _ => return false,
    }
    event.prevent_default();
    true
  }

  fn select_chip_before_caret(&self) {
    let Ok(selection) = self.controller.selection() else {
      return;
    };
    if selection.anchor.is_some_and(|anchor| anchor != selection.caret) {
      return;
    }
    let Some(chip) = self.draft.with_untracked(|text| ending_at(text, selection.caret)) else {
      return;
    };
    if let Err(error) = self.controller.select(chip.range.start, chip.range.end) {
      // Backspace then removes the chip's last character, as in any text: the mention is no chip any more.
      tracing::debug!(?error, "the chip before the caret could not be selected");
    }
  }

  fn close(&self, shown: &Shown) {
    self.closed.set(Some(shown.replaces.start));
    *self.shown.lock() = None;
  }

  /// Puts `mention` and a space in place of `replaces`, the `@` and what was typed after it, as one undo step, and
  /// the focus back in the field. When the field cannot take the edit (it is not shown, or an input method
  /// composes), the draft takes it.
  fn insert(&self, replaces: Range<usize>, mention: &Mention) {
    let written = format!("{} ", mention.written());
    let edited = self
      .controller
      .select(replaces.start, replaces.end)
      .and_then(|()| self.controller.replace_selection(&written));
    if !matches!(edited, Ok(true)) {
      tracing::debug!(?edited, "the field did not take the mention; the draft does");
      self.draft.update(|text| {
        if text.get(replaces.clone()).is_some() {
          text.replace_range(replaces, &written);
        }
      });
    }
    *self.shown.lock() = None;
    self.focus.set(0);
    self.refocus.set(true);
  }
}

/// The pieces of `text` from the start of the `@`'s line up to the `@` at `at`, in order, each saying whether it is a
/// mention the field draws as a chip.
fn before_on_line(text: &str, at: usize) -> Vec<(Range<usize>, bool)> {
  let start = text[..at].rfind('\n').map_or(0, |line| line + 1);
  let mut pieces = Vec::new();
  let mut from = start;
  for found in find(text) {
    let range = found.range;
    if range.start < from || range.end > at {
      continue;
    }
    if range.start > from {
      pieces.push((from..range.start, false));
    }
    from = range.end;
    pieces.push((range, true));
  }
  if from < at {
    pieces.push((from..at, false));
  }
  pieces
}

/// `words` as the field draws them: in its `typography`, or as a chip, Geist Mono 11 with 8 at its sides
/// ([`mention_chip`]). Spaces are kept as no-break spaces, which a line keeps at its end.
fn piece(words: &str, chip: bool, typography: TypographyStyle) -> Element {
  let kept = words.replace([' ', '\t'], "\u{a0}");
  if chip {
    Row::new()
      .flex_shrink(0.0)
      .padding(Padding::horizontal(SpacingSize::Sm))
      .child(Text::new(&kept).variant(ExtraTypography::MonoNote.role()).nowrap())
      .into()
  } else {
    Text::new(&kept).variant(typography).nowrap().flex_shrink(0.0).into()
  }
}
