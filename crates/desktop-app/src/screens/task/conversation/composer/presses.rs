//! What the composer's presses and keys do: send the draft as it is when pressed, the state's way or the
//! second where it has one, under the key its words are stored under, unless a send is under way; and the
//! field's keys. A press carries what it does as the person sees it (`Action`): sent, the task is read
//! again, and a run that moved on to offer something else refuses it (`outgoing`).

use std::sync::Arc;

use lurq::{app::events::keyboard_event::KeyboardEvent, core::Signal};
use orchester_core::{
  conversation::Recipient,
  id::{RunId, TaskId},
};
use orchester_workspace::{
  tasks::conversation::{
    composer::ComposerMode,
    outgoing::{Action, Context, Way},
  },
  workspace::OpenProject,
};

use super::{
  Press,
  controls::Sending,
  state::{ComposerState, Request},
};

/// Whose message a press sends, as the person reads the run: for `task`'s latest run `written_for`, `lost`
/// its runner, the mode's context `left`.
pub(super) struct Target {
  pub(super) project: OpenProject,
  pub(super) task: TaskId,
  pub(super) written_for: Option<RunId>,
  pub(super) lost: bool,
  pub(super) left: Option<Context>,
}

/// What sending does in `mode` while something is written and no send is under way: the state's way, and
/// the second where it has one.
pub(super) fn presses(state: &ComposerState, target: &Target, (mode, to): (&ComposerMode, Recipient)) -> Sending {
  if state.send.state().get().is_pending() || state.in_flight.get() {
    return Sending::Busy;
  }
  let Some(primary) = Action::of(mode, Way::Primary, to) else {
    return Sending::Empty;
  };
  if state.draft.get().trim().is_empty() {
    return Sending::Empty;
  }
  Sending::Ready {
    primary: press(state, target, (Way::Primary, primary)),
    second: Action::of(mode, Way::Second, to).map(|second| press(state, target, (Way::Second, second))),
  }
}

/// A press that sends the draft `way`, to do `expected`. It reads the draft and its key when pressed, and
/// does nothing while a send is under way: a second run of the action would replace the first mid-way.
fn press(state: &ComposerState, target: &Target, (way, expected): (Way, Action)) -> Press {
  let state = state.clone();
  let (project, task, written_for, reading) = (
    target.project.clone(),
    target.task,
    target.written_for,
    (target.lost, target.left),
  );
  Arc::new(move || {
    let text = state.draft.get_untracked();
    if text.trim().is_empty() || state.sending() {
      return;
    }
    state.send.run(Request {
      project: project.clone(),
      task,
      written_for,
      reading,
      way,
      expected,
      key: state.key_for(&text, (way, expected)),
      text,
    });
  })
}

/// What the field's keys do: Enter sends the state's way, Ctrl/Cmd+Enter the second where there is one,
/// Shift+Enter adds a line; Esc closes the send menu, then leaves the mode's context.
pub(super) fn keys(
  presses: &Sending,
  leave: Option<Press>,
  menu: Signal<bool>,
) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  let (primary, second) = match presses {
    Sending::Ready { primary, second } => (Some(primary.clone()), second.clone()),
    Sending::Empty | Sending::Busy => (None, None),
  };
  move |event: KeyboardEvent| {
    if event.key == "Escape" {
      if menu.get_untracked() {
        menu.set(false);
        event.prevent_default();
      } else if let Some(leave) = &leave {
        leave();
        event.prevent_default();
      }
      return;
    }
    if event.key != "Enter" || event.shift {
      return;
    }
    // Enter sends; it never also adds a line, even with nothing to send.
    event.prevent_default();
    // An Enter the input method passes on while it composes confirms the composition only.
    if event.composing {
      return;
    }
    let press = if event.ctrl || event.meta {
      second.clone().or_else(|| primary.clone())
    } else {
      primary.clone()
    };
    if let Some(press) = press {
      press();
    }
  }
}
