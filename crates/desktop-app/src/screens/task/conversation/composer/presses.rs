//! What the composer's presses and keys do: the state's way of sending and the second where it has one,
//! each storing the draft as it is when pressed, under the draft's key; a plan's changes as a decision on
//! the plan; and the field's keys.

use std::sync::Arc;

use lurq::{app::events::keyboard_event::KeyboardEvent, core::Signal};
use orchester_core::{
  conversation::{Recipient, Timing},
  run::ReviewNote,
};
use orchester_workspace::tasks::{
  conversation::composer::{ComposerMode, Then},
  detail::RunView,
  plan::PlanReview,
};

use super::{
  Composer, DockProps, Press,
  controls::Sending,
  send::{Outgoing, PlanAction, PlanChanges, SendAction},
};

impl Composer {
  /// What sending does in `mode` while something is written: the state's way, and the second where it
  /// has one; a plan's changes are a decision on the plan.
  pub(super) fn presses(
    &self,
    props: &DockProps,
    (mode, recipient): (&ComposerMode, Recipient),
    (sending, plan): (&SendAction, &PlanAction),
  ) -> Sending {
    let draft = props.composing.link.draft.clone();
    if draft.get().trim().is_empty() {
      return Sending::Empty;
    }
    if let ComposerMode::PlanChanges { .. } = mode {
      return match request_changes(props, plan) {
        Some(primary) => Sending::Ready { primary, second: None },
        None => Sending::Empty,
      };
    }
    let now = match mode {
      ComposerMode::Steer(working) if working.mid_turn => Timing::Now,
      _ => Timing::AfterTurn,
    };
    let press = |timing: Timing, then: Then| -> Press {
      let outgoing = self.outgoing(props, mode, (recipient, timing, then));
      let (action, draft, last) = (sending.clone(), draft.clone(), self.last.clone());
      Arc::new(move || {
        let text = draft.get_untracked();
        if text.trim().is_empty() {
          return;
        }
        let outgoing = Outgoing {
          text,
          ..outgoing.clone()
        };
        *last.lock() = Some(outgoing.clone());
        action.run(outgoing);
      })
    };
    match mode.then() {
      Some(then) => Sending::Ready {
        primary: press(now, then),
        second: mode.then_later().map(|later| press(Timing::AfterTurn, later)),
      },
      None => Sending::Empty,
    }
  }

  /// The message `mode` sends to `recipient`, as `timing` and `then` say, but its words.
  fn outgoing(
    &self,
    props: &DockProps,
    mode: &ComposerMode,
    (recipient, timing, then): (Recipient, Timing, Then),
  ) -> Outgoing {
    let detail = &props.composing.detail;
    let run = detail.run.as_ref();
    Outgoing {
      project: props.composing.project.open.clone(),
      key: self.key.get().0,
      task: detail.task.id(),
      run: run.map(RunView::id),
      // Only a message that continues the run carries the failure it continues after.
      recovery_after: props
        .conversation
        .recovery_after
        .filter(|_| *mode == ComposerMode::Continue),
      recipient,
      timing,
      then,
      pipeline: run.map(|run| run.pipeline.clone()).unwrap_or_default(),
      text: String::new(),
    }
  }
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

/// *Request changes* with the draft as the changes the waiting plan needs.
fn request_changes(props: &DockProps, plan: &PlanAction) -> Option<Press> {
  let run = props.composing.detail.run.as_ref()?;
  let review = PlanReview::of(run)?;
  let waiting = review.waiting();
  let (project, execution, version) = (props.composing.project.open.clone(), waiting.execution, waiting.version);
  let (draft, plan, run) = (props.composing.link.draft.clone(), plan.clone(), review.run);
  Some(Arc::new(move || {
    // A blank note is no change: the press waits for words.
    let Ok(note) = ReviewNote::new(&draft.get_untracked()) else {
      return;
    };
    plan.run(PlanChanges {
      project: project.clone(),
      run,
      execution,
      version,
      note,
    });
  }))
}
