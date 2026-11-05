//! What the ORC-732 states change of Kontur's tasks and modules on the stand-in team server, what it meets first, and
//! what this computer holds once it joined (`Team authority · redesign` `BZc66` rows 2–5 and 7): the claims it took
//! here, taken through the dispatcher as the Task page takes them; the team runs the server fenced, as this computer
//! keeps them; and the module branch a delivery takes. Fixture data for QA, never shipped.

use std::{collections::HashMap, error::Error, path::Path, process::Command};

use orchester_api::{
  test_support::team_server::{AuthorityClaim, AuthorityModule, AuthorityTask, Person, Plan, TeamServer},
  v2::Subject,
};
use orchester_app::services::Services;
use orchester_core::{
  id::{ClaimId, ModuleId, OperationId, ServerId, TaskId},
  store::RunStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::actions::{Action, ActionOutcome, ClaimAction, Dispatcher, Intent, TeamOutcome};
use tokio::runtime::Handle;

use super::State;

const MARA: Person = Person::Named("mara");
const OLHA: Person = Person::Named("olha");

/// A claim of `holder` last renewed `minutes` ago, stale or not.
const fn claim(holder: Person, minutes: i64, stale: bool) -> Option<AuthorityClaim> {
  Some(AuthorityClaim {
    holder,
    renewed_minutes_ago: minutes,
    stale,
    generation: 0,
  })
}

/// The tasks of `team-claims` with a claim of their own, by number: its claim and the state of its work.
const CLAIMS: [(u32, Option<AuthorityClaim>, Option<&str>); 9] = [
  (21, claim(Person::You, 3, false), Some("awaiting_human")),
  (14, claim(Person::You, 4, false), Some("succeeded")),
  (11, claim(MARA, 1, false), Some("running")),
  (23, claim(Person::You, 1, false), Some("running")),
  (24, claim(Person::You, 2, false), Some("paused")),
  (13, claim(MARA, 1, false), Some("running")),
  (10, claim(Person::You, 1, false), Some("running")),
  (16, claim(MARA, 22, true), Some("paused")),
  (5, None, None),
];

/// The tasks of `team-claims` this computer took the claim of: those with runs here, and the one it holds stale.
const HELD_HERE: [u32; 4] = [12, 21, 14, 24];

/// The team runs of `team-claims` the server fenced, by their task's number, with why.
const FENCED: [(u32, &str); 2] = [(11, "claim_lost"), (23, "session_mismatch")];

/// The index of Billing exports among the modules, the person's on this computer.
const BILLING: usize = 1;

/// `task` as `state` has it on the server.
pub fn task(state: State, task: &mut AuthorityTask) {
  if state != State::Claims {
    return;
  }
  if let Some((_, claim, work)) = CLAIMS.iter().find(|(number, ..)| *number == task.number) {
    task.claim = *claim;
    task.work = *work;
    task.assignee = claim.map(|claim| claim.holder);
  }
}

/// `modules` as `state` has them on the server.
pub fn modules(state: State, modules: &mut [AuthorityModule]) {
  if matches!(state, State::ModuleWork | State::Claims)
    && let Some(webhooks) = modules.first_mut()
  {
    webhooks.claim = claim(OLHA, 41, true);
  }
}

/// What the server meets in `state` before its own rule.
pub fn plan(state: State, server: &TeamServer) {
  match state {
    State::Claims => server.plan("claim.take_over", Plan::RenewedFirst),
    State::ModuleWork => server.plan("claim.take_over", Plan::Refuse(409, "transition_denied")),
    State::AdminWork => {
      server.plan("module.branch.set", Plan::Refuse(422, "invalid"));
      server.plan("module.branch.set", Plan::Refuse(409, "transition_denied"));
      server.plan(
        "module.branch.set",
        Plan::BranchFirst {
          by: OLHA,
          branch: "module/webhooks-signing",
        },
      );
      server.automatic_merge(true);
      server.automatic_merge_changed_first(true);
    }
    _ => {}
  }
}

/// What this computer holds in `state` once Kontur at `root` joined `server` as `id`: the claims it took here, the team
/// runs the server fenced, and the module branch a delivery takes; `ids` are Kontur's tasks here by number.
pub fn after_join(
  state: State,
  (server, id): (&TeamServer, ServerId),
  (services, runtime): (&Services, &Handle),
  (root, ids): (&Path, &HashMap<u32, String>),
) -> Result<(), Box<dyn Error>> {
  let project = server.project("kontur").ok_or("the team shares Kontur")?;
  let session = runtime.block_on(services.servers.team_session(id, project))?;
  let dispatcher = Dispatcher::new(root.to_owned(), services.servers.clone(), Some(session));
  let take = |subject| claim_here(&dispatcher, runtime, subject);
  match state {
    State::MemberRunning | State::ModuleOutage | State::TeamRunning => {
      let claim = take(task_subject(ids, 12)?)?;
      if state == State::TeamRunning
        && let Some(task) = local(ids, 12)?
      {
        team_run(root, (task, claim), None)?;
      }
    }
    State::Claims => {
      for number in HELD_HERE {
        take(task_subject(ids, number)?)?;
      }
      server.claim_aged(24, 22, true);
      for (number, fence) in FENCED {
        if let Some(task) = local(ids, number)? {
          team_run(root, (task, ClaimId::generate()), Some(fence))?;
        }
      }
    }
    State::ModuleWork | State::AdminWork => {
      take(Subject::Module {
        id: module_id(BILLING)?,
      })?;
      branch(root, "module/billing-exports")?;
    }
    _ => {}
  }
  Ok(())
}

/// Takes `subject`'s claim through the dispatcher, as the page does; the claim the receipt names.
fn claim_here(dispatcher: &Dispatcher, runtime: &Handle, subject: Subject) -> Result<ClaimId, Box<dyn Error>> {
  let intent = Intent::new(Action::Claim(ClaimAction::Acquire { subject }));
  // The dispatcher holds the project database across its awaits: it runs here, on the thread that sets the state up.
  let outcome = runtime.block_on(dispatcher.dispatch(&intent))?;
  let ActionOutcome::Team(TeamOutcome::Completed(orchester_workspace::actions::Accepted::Command(receipt))) = outcome
  else {
    return Err(format!("the stand-in did not give the claim: {outcome:?}").into());
  };
  match receipt.result {
    orchester_api::v2::TeamCommandValue::Claim { claim, .. } => Ok(claim.id),
    other => Err(format!("the stand-in answered a claim with {other:?}").into()),
  }
}

/// The id here of the task numbered `number`, when this computer has it.
fn local(ids: &HashMap<u32, String>, number: u32) -> Result<Option<TaskId>, Box<dyn Error>> {
  Ok(ids.get(&number).map(|id| id.parse()).transpose()?)
}

/// The task numbered `number` as the server names it: by its id here when this computer has it, else by the id the
/// stand-in makes from its number.
fn task_subject(ids: &HashMap<u32, String>, number: u32) -> Result<Subject, Box<dyn Error>> {
  let id = match local(ids, number)? {
    Some(id) => id,
    None => format!("0191c7c0-0000-7000-8000-0000001{number:05x}").parse()?,
  };
  Ok(Subject::Task { id })
}

/// The id of the module at `index` of those the stand-in serves.
fn module_id(index: usize) -> Result<ModuleId, Box<dyn Error>> {
  Ok(format!("0191c7c0-0000-7000-8000-0000002{index:05x}").parse()?)
}

/// Records the latest run of `task` as a team run under `claim`, as a start through the team server leaves it, fenced
/// for `fence` when one is given: the records `team_work` keeps of a start that reached its run.
fn team_run(root: &Path, (task, claim): (TaskId, ClaimId), fence: Option<&str>) -> Result<(), Box<dyn Error>> {
  let run = ProjectDb::open(root)?
    .runs_of_task(task)?
    .into_iter()
    .last()
    .ok_or("the seed gave the task a run")?
    .map_err(|_| "the seed's run reads")?;
  let (operation, work, now) = (
    OperationId::generate(),
    ClaimId::generate(),
    Timestamp::now().unix_millis(),
  );
  let connection = rusqlite::Connection::open(root.join(".orchester").join("orchester.db"))?;
  connection.execute(
    "INSERT INTO team_starts (operation_id, task_id, request, stage, work_id, bundle_sha256, run_id, created_at, \
     updated_at) VALUES (?1, ?2, '{}', 'started', ?3, ?4, ?5, ?6, ?6)",
    rusqlite::params![
      operation.to_string(),
      task.to_string(),
      work.to_string(),
      "0".repeat(64),
      run.id().to_string(),
      now
    ],
  )?;
  connection.execute(
    "INSERT INTO team_runs (run_id, operation_id, team_task_id, claim_id, work_id, next_sequence, fence, fenced_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)",
    rusqlite::params![
      run.id().to_string(),
      operation.to_string(),
      task.to_string(),
      claim.to_string(),
      work.to_string(),
      fence,
      fence.map(|_| now)
    ],
  )?;
  Ok(())
}

/// Makes `branch` at the checkout's head, as an Admin's first assignment of the module named it. Only in a repository
/// whose top is the checkout itself and that has a commit: a headless test's checkout is none, and git would find the
/// repository of a folder above it instead, which is never this scenario's to change.
fn branch(root: &Path, branch: &str) -> Result<(), Box<dyn Error>> {
  let git = || {
    let mut git = Command::new("git");
    git.arg("-C").arg(root);
    git
  };
  let top = git().args(["rev-parse", "--show-toplevel"]).output()?;
  let top = String::from_utf8_lossy(&top.stdout);
  let ours = !top.trim().is_empty() && Path::new(top.trim()).canonicalize().ok() == root.canonicalize().ok();
  if !ours {
    return Ok(());
  }
  let head = git().args(["rev-parse", "--verify", "--quiet", "HEAD"]).output()?;
  if !head.status.success() {
    return Ok(());
  }
  let made = git().args(["branch", branch]).status()?;
  if !made.success() {
    return Err(format!("git did not make {branch}").into());
  }
  Ok(())
}
