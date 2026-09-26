//! What the runner works on, for the desktop's status bar and close confirmation: each executing
//! run with its current step, agent and model, and the agent slots. Every change is announced as
//! an [`Event::WorkSummary`] once the desktop has said how many slots there are.

use std::num::NonZeroU32;

use orchester_core::run::Run;
use orchester_ipc::{Event, RunWork, WorkState, WorkSummary};
use parking_lot::Mutex;

use super::Notifier;

pub(super) struct Board {
  notifier: Notifier,
  state: Mutex<State>,
}

#[derive(Default)]
struct State {
  /// Unknown until the desktop attaches; no summary is sent before that.
  slots: Option<NonZeroU32>,
  /// The runner stops at checkpoints: runs that settle stay listed as saved.
  stopping: bool,
  runs: Vec<RunWork>,
}

impl Board {
  pub(super) fn new(notifier: Notifier) -> Self {
    Self {
      notifier,
      state: Mutex::new(State::default()),
    }
  }

  pub(super) fn slots(&self) -> Option<NonZeroU32> {
    self.state.lock().slots
  }

  pub(super) fn set_slots(&self, slots: NonZeroU32) {
    self.change(|state| state.slots = Some(slots));
  }

  pub(super) fn is_stopping(&self) -> bool {
    self.state.lock().stopping
  }

  /// The runner stops at checkpoints: every listed run is saving from now on.
  pub(super) fn stop_at_checkpoints(&self) {
    self.change(|state| {
      state.stopping = true;
      for run in &mut state.runs {
        run.state = WorkState::Saving;
      }
    });
  }

  /// `work` is what its run works on now.
  pub(super) fn working(&self, mut work: RunWork) {
    self.change(|state| {
      if state.stopping {
        work.state = WorkState::Saving;
      }
      match state.runs.iter_mut().find(|listed| listed.run == work.run) {
        Some(listed) => *listed = work,
        None => state.runs.push(work),
      }
    });
  }

  /// `run` no longer executes. While the runner stops at checkpoints it stays listed as saved,
  /// at its last checkpoint; otherwise it leaves the list.
  pub(super) fn settled(&self, run: &Run) {
    self.change(|state| {
      if !state.stopping {
        state.runs.retain(|listed| listed.run != run.id());
        return;
      }
      if let Some(listed) = state.runs.iter_mut().find(|listed| listed.run == run.id()) {
        listed.state = WorkState::Saved {
          checkpoint: run
            .last_checkpoint()
            .map(|checkpoint| checkpoint.revision().as_str().to_owned()),
        };
      }
    });
  }

  /// Announces the current summary again, e.g. right after attaching.
  pub(super) fn announce(&self) {
    self.change(|_| {});
  }

  fn change(&self, change: impl FnOnce(&mut State)) {
    let summary = {
      let mut state = self.state.lock();
      change(&mut state);
      state.slots.map(|agent_slots| WorkSummary {
        agent_slots,
        runs: state.runs.clone(),
      })
    };
    if let Some(summary) = summary {
      self.notifier.send(Event::WorkSummary(summary));
    }
  }
}
