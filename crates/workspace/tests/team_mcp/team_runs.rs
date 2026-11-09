//! A team task's run as this computer keeps it: the task's identity and the run with the inputs the team server
//! accepted, and no definition of the task (no copy the old sharing stream cached).

use orchester_core::{
  connection::AgentProvider,
  id::{RunId, StepExecutionId},
  integration::base::RunBaseSource,
  revision::Revision,
  run::{BranchName, NewRun, Run, RunEvent, RunStatus},
  store::{ConversationStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, TeamFence};
use serde_json::json;

use crate::{Checkout, team_server::TeamServer, tools::server};

/// A checkout linked to the fake team server whose team task `KON-7` was started through `run_task` and whose run the
/// runner then recorded, running.
pub(crate) struct Started {
  pub(crate) checkout: Checkout,
  pub(crate) server: TeamServer,
  pub(crate) run: RunId,
}

pub(crate) fn started() -> Started {
  started_with(true)
}

/// [`started`] for a runner that crashed after creating the run and before recording it as the team start's: the run
/// keeps the accepted inputs and no team record.
pub(crate) fn started_without_record() -> Started {
  started_with(false)
}

fn started_with(record: bool) -> Started {
  let (server, _seen) = server();
  let config = server.state().config.clone();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    crate::ready::team_ready(db, &config, AgentProvider::CodexCli);
  });
  // The session has no runner: the start is accepted and its inputs kept for the runner.
  assert_eq!(
    checkout.refused("run_task", &json!({"project": "Kontur", "key": "KON-7"})),
    "runner_unavailable"
  );
  let run = record_run(&mut checkout.db(), record);
  assert!(
    checkout.db().tasks().expect("tasks").is_empty(),
    "the team task has no definition here"
  );
  Started { checkout, server, run }
}

/// Records the run of the one team start the way the runner does, and starts it; with `record`, records it as the
/// start's run too.
fn record_run(db: &mut ProjectDb, record: bool) -> RunId {
  let operation = db.open_team_starts().expect("starts")[0].operation;
  let accepted = db
    .accepted_team_inputs(operation)
    .expect("read")
    .expect("the accepted inputs");
  let orchester_core::run::inputs::InputsOrigin::Team(acceptance) = accepted.inputs.bundle().origin().clone() else {
    panic!("team inputs");
  };
  let snapshot = accepted.inputs.bundle().inputs().snapshot().with_sources(
    &db.project_sources().expect("sources"),
    &db.task_sources(accepted.task).expect("sources"),
  );
  let new = NewRun {
    task: accepted.task,
    snapshot,
    branch: BranchName::new("orchester/kon-7-run-1").expect("branch"),
    base: Revision::new(acceptance.base_commit.as_str()).expect("revision"),
    base_branch: Some(acceptance.branch),
    base_source: RunBaseSource::Module {
      module: acceptance.module,
    },
    budget: None,
    max_duration: None,
  };
  let mut run = db
    .create_run_with_inputs(operation.run_key(), new, &accepted.inputs, (Timestamp::now(), None))
    .expect("the run");
  let started = run.start(Timestamp::now()).expect("started");
  db.append_events(&run, &started).expect("recorded");
  if record {
    db.record_team_start_run(operation, run.id(), Timestamp::now())
      .expect("the team run");
  }
  run.id()
}

impl Started {
  /// Applies `transition` to the run and stores what it appended.
  pub(crate) fn settle(&self, transition: impl FnOnce(&mut Run, Timestamp) -> Vec<RunEvent>) -> RunStatus {
    let mut db = self.checkout.db();
    let mut run = db.run(self.run).expect("read").expect("the run");
    let events = transition(&mut run, Timestamp::now());
    db.append_events(&run, &events).expect("recorded");
    run.status().clone()
  }
}

impl Started {
  /// The first step of the run starts, then `next` puts it in the state a person or an agent answers.
  pub(crate) fn step_into(&self, next: impl FnOnce(&mut Run, StepExecutionId, Timestamp) -> Vec<RunEvent>) {
    self.settle(|run, now| {
      let execution = run.current_attempt().expect("attempt").executions()[0].id();
      let mut events = run.start_step(execution, now).expect("step started");
      events.extend(next(run, execution, now));
      events
    });
  }

  /// The team server fenced the run: it goes on no more here.
  pub(crate) fn fence(&self) {
    self
      .checkout
      .db()
      .fence_team_run(self.run, TeamFence::ClaimLost, Timestamp::now())
      .expect("fenced");
  }

  /// How many messages the team task's conversation keeps.
  pub(crate) fn messages(&self) -> usize {
    let task = self.checkout.db().run(self.run).expect("read").expect("run").task();
    self.checkout.db().messages(task).expect("messages").len()
  }
}
