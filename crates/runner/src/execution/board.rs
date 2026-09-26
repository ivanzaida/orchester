//! What the runner works on, for the desktop's status bar and close confirmation, and the agent
//! slots it shares between runs: each agent at work — a step execution of an executing run, with
//! its task, step, agent and model — takes one slot, so a run's parallel steps and other runs
//! compete for the same slots. Every change is announced as an [`Event::WorkSummary`] once the
//! desktop has said how many slots there are.

use std::num::NonZeroU32;

use orchester_core::{
  id::{RunId, StepExecutionId},
  run::{Run, RunStatus},
};
use orchester_ipc::{Event, RunWork, WorkState, WorkSummary};
use parking_lot::Mutex;

use super::Notifier;

pub(super) struct Board {
  notifier: Notifier,
  state: Mutex<State>,
}

#[derive(Default)]
struct State {
  /// Unknown until the desktop attaches; no summary is sent and slots are not bounded before that.
  slots: Option<NonZeroU32>,
  /// The runner stops at checkpoints: runs that settle stay listed as saved.
  stopping: bool,
  agents: Vec<Agent>,
}

/// One slot in use. `work` is `None` for an agent whose profile or connection is gone: it still
/// holds its slot until its step fails, but has nothing to show.
struct Agent {
  run: RunId,
  execution: Option<StepExecutionId>,
  work: Option<RunWork>,
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

  /// Whether an agent slot is free.
  pub(super) fn has_free_slot(&self) -> bool {
    self.state.lock().free() > 0
  }

  pub(super) fn is_stopping(&self) -> bool {
    self.state.lock().stopping
  }

  /// The runner stops at checkpoints: every listed agent is saving from now on.
  pub(super) fn stop_at_checkpoints(&self) {
    self.change(|state| {
      state.stopping = true;
      for work in state.agents.iter_mut().filter_map(|agent| agent.work.as_mut()) {
        work.state = WorkState::Saving;
      }
    });
  }

  /// Takes a slot for `execution` of `run`, which shows as `work`; `false` when none is free.
  pub(super) fn take(&self, run: RunId, execution: StepExecutionId, mut work: Option<RunWork>) -> bool {
    let mut taken = false;
    self.change(|state| {
      if state.free() == 0 {
        return;
      }
      if let Some(work) = &mut work
        && state.stopping
      {
        work.state = WorkState::Saving;
      }
      state.agents.push(Agent {
        run,
        execution: Some(execution),
        work,
      });
      taken = true;
    });
    taken
  }

  /// `execution`'s agent ended: its slot is free again. While the runner stops at checkpoints it
  /// stays listed until its run settles, which shows it as saved.
  pub(super) fn release(&self, execution: StepExecutionId) {
    self.change(|state| {
      if state.stopping {
        for agent in state
          .agents
          .iter_mut()
          .filter(|agent| agent.execution == Some(execution))
        {
          agent.execution = None;
        }
      } else {
        state.agents.retain(|agent| agent.execution != Some(execution));
      }
    });
  }

  /// `run` no longer executes. While the runner stops at checkpoints a run that settled stays
  /// listed as saved, at its last checkpoint, without holding a slot; otherwise it leaves the
  /// list. A run still recorded as running did not settle (its task failed), so it is never shown
  /// as saved.
  pub(super) fn settled(&self, run: &Run) {
    self.change(|state| {
      let listed: Vec<Agent> = state.agents.extract_if(.., |agent| agent.run == run.id()).collect();
      if !state.stopping || run.status() == &RunStatus::Running {
        return;
      }
      let checkpoint = run
        .last_checkpoint()
        .map(|checkpoint| checkpoint.revision().as_str().to_owned());
      let saved = listed.into_iter().filter_map(|agent| agent.work).map(|mut work| {
        work.state = WorkState::Saved {
          checkpoint: checkpoint.clone(),
        };
        Agent {
          run: run.id(),
          execution: None,
          work: Some(work),
        }
      });
      state.agents.extend(saved);
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
        runs: state.agents.iter().filter_map(|agent| agent.work.clone()).collect(),
      })
    };
    if let Some(summary) = summary {
      self.notifier.send(Event::WorkSummary(summary));
    }
  }
}

impl State {
  fn free(&self) -> usize {
    let Some(slots) = self.slots else {
      return usize::MAX;
    };
    let busy = self.agents.iter().filter(|agent| agent.execution.is_some()).count();
    (slots.get() as usize).saturating_sub(busy)
  }
}
