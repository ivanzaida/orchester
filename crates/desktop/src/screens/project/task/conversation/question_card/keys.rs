//! The question card's keys. The card listens to every key but acts only while the focus is on one of its
//! rows or fields, which Tab or a click put there: it never takes the focus itself. A letter picks its option
//! (the letter after the last focuses the own answer's field), ↑ ↓ move between the rows of every question,
//! Space presses the focused row, Enter answers; in the field, Enter answers and Esc or ↑ go back to the
//! options. Any key marks that the keyboard is in use, so the focused row shows the ring; a press with the
//! pointer clears it.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, events::KeyboardEvent};

use super::{CardState, Fields};
use crate::tasks::question::{letter, option_of_key};

/// A row of a question: one of its options, or its own answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RowAt {
  Option(usize),
  Own,
}

/// A row of the card: its question and the row in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Spot {
  pub(super) question: usize,
  pub(super) row: RowAt,
}

impl lurq::app::component::DevtoolsInspectable for Spot {}

/// Where the focus is among the card's rows and fields.
fn focused_spot(fields: &Fields) -> Option<Spot> {
  let own = fields
    .inputs
    .iter()
    .position(lurq::core::ElementRef::focused)
    .map(|question| Spot {
      question,
      row: RowAt::Own,
    });
  own.or_else(|| {
    fields.rows.iter().enumerate().find_map(|(question, rows)| {
      rows
        .iter()
        .position(lurq::core::ElementRef::focused)
        .map(|option| Spot {
          question,
          row: RowAt::Option(option),
        })
    })
  })
}

/// The card's key handler.
pub(super) fn card_keys(state: CardState) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  move |event: KeyboardEvent| {
    // Tab may bring the focus in from outside: its row then shows the ring.
    if event.key == "Tab" {
      state.set_keyed(true);
      return;
    }
    let Some(spot) = focused_spot(&state.fields.lock()) else {
      return;
    };
    if event.ctrl || event.alt || event.meta {
      return;
    }
    state.set_keyed(true);
    match spot.row {
      RowAt::Own => own_key(&state, spot.question, &event),
      RowAt::Option(option) => option_key(&state, (spot.question, option), &event),
    }
  }
}

fn own_key(state: &CardState, question: usize, event: &KeyboardEvent) {
  match event.key.as_str() {
    "Enter" => {
      event.prevent_default();
      if !event.composing {
        state.submit();
      }
    }
    "Escape" | "ArrowUp" => {
      event.prevent_default();
      let last = options_of(state, question).saturating_sub(1);
      move_to(state, question, RowAt::Option(last));
    }
    "ArrowDown" => {
      event.prevent_default();
      move_to_next(
        state,
        Spot {
          question,
          row: RowAt::Own,
        },
      );
    }
    _ => {}
  }
}

fn option_key(state: &CardState, (question, option): (usize, usize), event: &KeyboardEvent) {
  let spot = Spot {
    question,
    row: RowAt::Option(option),
  };
  match event.key.as_str() {
    "Enter" => {
      event.prevent_default();
      state.submit();
    }
    "ArrowDown" => {
      event.prevent_default();
      move_to_next(state, spot);
    }
    "ArrowUp" => {
      event.prevent_default();
      move_to_previous(state, spot);
    }
    key => {
      let count = options_of(state, question);
      let Some(index) = option_of_key(key, count) else {
        return;
      };
      event.prevent_default();
      if index < count {
        let questions = state.request.questions();
        state.draft.update(|draft| draft.pick(questions, (question, index)));
        move_to(state, question, RowAt::Option(index));
      } else {
        move_to(state, question, RowAt::Own);
      }
    }
  }
}

fn options_of(state: &CardState, question: usize) -> usize {
  state
    .request
    .questions()
    .get(question)
    .map_or(0, |question| question.options().len())
}

fn move_to(state: &CardState, question: usize, row: RowAt) {
  state.focus_to.set(Some(Spot { question, row }));
}

/// The row after `spot`: the next option, the own answer after the last, then the next question's first
/// option.
fn move_to_next(state: &CardState, spot: Spot) {
  let count = options_of(state, spot.question);
  match spot.row {
    RowAt::Option(option) if option + 1 < count => move_to(state, spot.question, RowAt::Option(option + 1)),
    RowAt::Option(_) => move_to(state, spot.question, RowAt::Own),
    RowAt::Own if spot.question + 1 < state.request.questions().len() => {
      move_to(state, spot.question + 1, RowAt::Option(0));
    }
    RowAt::Own => {}
  }
}

/// The row before `spot`: the previous option, then the previous question's own answer.
fn move_to_previous(state: &CardState, spot: Spot) {
  match spot.row {
    RowAt::Option(option) if option > 0 => move_to(state, spot.question, RowAt::Option(option - 1)),
    RowAt::Option(_) if spot.question > 0 => move_to(state, spot.question - 1, RowAt::Own),
    RowAt::Option(_) => {}
    RowAt::Own => move_to(
      state,
      spot.question,
      RowAt::Option(options_of(state, spot.question).saturating_sub(1)),
    ),
  }
}

/// The keys' hint under the rows: in an own answer's field, how to answer or go back; elsewhere, which
/// letters pick and how to move, pick or check, and answer.
pub(super) fn hint(ctx: &Ctx, state: &CardState) -> Arc<str> {
  if matches!(state.focused.get(), Some(Spot { row: RowAt::Own, .. })) {
    return ctx.t("runs.conversation.question.hintField");
  }
  let questions = state.request.questions();
  let rows = questions
    .iter()
    .map(|question| question.options().len())
    .max()
    .unwrap_or(0);
  let last = letter(rows).map_or_else(String::new, String::from);
  let key = if questions.iter().any(orchester_core::question::Question::multi_select) {
    "runs.conversation.question.hintMulti"
  } else {
    "runs.conversation.question.hintSingle"
  };
  ctx.t_args(key, [("first", "A"), ("last", last.as_str())])
}
