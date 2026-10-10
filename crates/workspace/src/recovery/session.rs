//! This session's start-up recovery (layer "App lifecycle" `Ob1P7`, *Startup after a crash* `JSRBO` and
//! `AgD9q`): after Quit now or a crash the runs the previous session left interrupted are read once, in
//! the background, and each project's Overview offers its own runs, all or none. Nothing continues on
//! its own; a project's runs not continued stay interrupted and each is continued later from its task
//! page. After "Wait for agents to save" nothing is offered: those runs are paused.

use std::{collections::BTreeMap, sync::Arc};

use orchester_core::{
  id::{ProjectId, RunId},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::SessionEnd;
use tokio::{runtime::Handle, sync::watch};

use super::{InterruptedRun, interrupted_runs, reports};
use crate::{
  background,
  messages::{self, Message},
  runner::{Continued, RunRequestError, Runners},
  sharing::{Servers, project::WorkClaim},
  tasks::run::control_message,
  workspace::Workspace,
};

/// What the recovery read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reading {
  /// The previous session ended cleanly or after "Wait for agents to save": nothing to offer.
  Nothing,
  Reading,
  /// The runs could not be read; they stay interrupted and each can be continued from its task page.
  Failed(Message),
  Read(Arc<[InterruptedRun]>),
}

/// What the person decided for one project's interrupted runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
  Undecided,
  /// *Continue all*: the runner takes the runs back from their checkpoints.
  Continuing,
  Continued(Continuation),
  /// *Don't continue*: the runs stay interrupted.
  Left,
}

/// How continuing a project's interrupted runs went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Continuation {
  /// How many runs continued.
  pub continued: usize,
  /// The runs that did not, with why; each stays interrupted.
  pub not_continued: Vec<NotContinued>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotContinued {
  pub run: InterruptedRun,
  pub reason: Message,
}

/// The recovery's state: what was read, and each project's decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryState {
  pub reading: Reading,
  /// When the previous session ended this way, as the notice says.
  pub previous: SessionEnd,
  decisions: Arc<BTreeMap<ProjectId, Decision>>,
}

impl RecoveryState {
  /// The interrupted runs of `project`, in the order of their numbers; none until they are read.
  pub fn runs_of(&self, project: ProjectId) -> Vec<InterruptedRun> {
    match &self.reading {
      Reading::Read(runs) => runs.iter().filter(|run| run.project_id == project).cloned().collect(),
      Reading::Nothing | Reading::Reading | Reading::Failed(_) => Vec::new(),
    }
  }

  pub fn decision(&self, project: ProjectId) -> Decision {
    self.decisions.get(&project).cloned().unwrap_or(Decision::Undecided)
  }
}

/// The session's recovery; clones share it.
#[derive(Clone)]
pub struct Recovery {
  state: watch::Sender<RecoveryState>,
  runtime: Handle,
}

impl PartialEq for Recovery {
  fn eq(&self, other: &Self) -> bool {
    self.state.same_channel(&other.state)
  }
}

impl Recovery {
  /// Starts reading the runs `previous`, the end of the last session, left interrupted in the
  /// registered projects, on `runtime`, when that end is one the recovery reports; this session
  /// started at `since`.
  pub fn start(
    (workspace, programs): (Workspace, Programs),
    previous: SessionEnd,
    since: Timestamp,
    runtime: &Handle,
  ) -> Self {
    let reports = reports(previous);
    let recovery = Self {
      state: watch::Sender::new(RecoveryState {
        reading: if reports { Reading::Reading } else { Reading::Nothing },
        previous,
        decisions: Arc::default(),
      }),
      runtime: runtime.clone(),
    };
    if reports {
      let state = recovery.state.clone();
      runtime.spawn(async move {
        let read = background::run(move || interrupted_runs(&workspace, &programs, previous, since)).await;
        let reading = match read {
          Ok(Ok(runs)) => Reading::Read(runs.into()),
          Ok(Err(error)) => {
            tracing::error!(%error, "cannot read the interrupted runs");
            Reading::Failed(messages::store_error(&error))
          }
          Err(message) => Reading::Failed(message),
        };
        state.send_modify(|state| state.reading = reading);
      });
    }
    recovery
  }

  /// The state, whenever it changes.
  pub fn subscribe(&self) -> watch::Receiver<RecoveryState> {
    self.state.subscribe()
  }

  pub fn state(&self) -> RecoveryState {
    self.state.borrow().clone()
  }

  /// *Don't continue*: `project`'s runs stay interrupted and the notice goes.
  pub fn leave(&self, project: ProjectId) {
    self.decide(project, |decision| {
      matches!(decision, Decision::Undecided).then_some(Decision::Left)
    });
  }

  /// *Continue all*: continues every interrupted run of `project` from its checkpoint through its
  /// runner, started in the background when no tab shows the project. A run of a shared task
  /// continues only under the person's claim.
  pub fn continue_all(&self, project: ProjectId, runners: &Runners, servers: &Servers) {
    let started = self.decide(project, |decision| {
      matches!(decision, Decision::Undecided).then_some(Decision::Continuing)
    });
    if !started {
      return;
    }
    let runs = self.state().runs_of(project);
    let (recovery, runners, servers) = (self.clone(), runners.clone(), servers.clone());
    self.runtime.spawn(async move {
      let continuation = continue_runs(&runners, &servers, &runs).await;
      tracing::info!(
        continued = continuation.continued,
        not_continued = continuation.not_continued.len(),
        "continued interrupted runs"
      );
      recovery.decide(project, |_| Some(Decision::Continued(continuation)));
    });
  }

  /// Sets `project`'s decision to what `next` makes of it; whether it changed.
  fn decide(&self, project: ProjectId, next: impl FnOnce(&Decision) -> Option<Decision>) -> bool {
    self.state.send_if_modified(|state| {
      let Some(decision) = next(&state.decision(project)) else {
        return false;
      };
      Arc::make_mut(&mut state.decisions).insert(project, decision);
      true
    })
  }
}

/// Continues `runs`, all of one project, and says which did not, with why. A claim taken for a run
/// that did not continue is released.
async fn continue_runs(runners: &Runners, servers: &Servers, runs: &[InterruptedRun]) -> Continuation {
  let mut not_continued = Vec::new();
  let mut claimed: Vec<(&InterruptedRun, WorkClaim)> = Vec::new();
  for run in runs {
    match servers.claim_for_work(&run.project_root, run.task).await {
      Ok(claim) => claimed.push((run, claim)),
      Err(refusal) => not_continued.push(NotContinued {
        run: run.clone(),
        reason: refusal.message(),
      }),
    }
  }
  let Some((root, name)) = claimed
    .first()
    .map(|(run, _)| (run.project_root.clone(), run.project.clone()))
  else {
    return Continuation {
      continued: 0,
      not_continued,
    };
  };
  let ids: Vec<RunId> = claimed.iter().map(|(run, _)| run.run).collect();
  let answer = runners.continue_runs(&root, &name, ids).await;
  if let Err(error) = &answer {
    tracing::warn!(project = %root.display(), %error, "the interrupted runs did not continue");
  }
  let mut continued = 0;
  for (run, claim) in claimed {
    match refusal(&answer, run.run) {
      None => continued += 1,
      Some(reason) => {
        claim.release(servers).await;
        not_continued.push(NotContinued {
          run: run.clone(),
          reason,
        });
      }
    }
  }
  Continuation {
    continued,
    not_continued,
  }
}

/// Why the runner did not continue `run`, given its `answer`; `None` when it did.
fn refusal(answer: &Result<Continued, RunRequestError>, run: RunId) -> Option<Message> {
  match answer {
    Ok(answer) => answer
      .refused
      .iter()
      .find(|refused| refused.run == run)
      .map(|refused| control_message(&RunRequestError::Rejected(refused.rejection.clone()))),
    Err(error) => Some(control_message(error)),
  }
}
