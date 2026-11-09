//! What the ORC-732 and ORC-735 states change of Kontur's tasks and modules on the stand-in team server, what it meets
//! first, and what this computer holds once it joined (`Team authority · redesign` `BZc66` rows 2–5 and 7): the claims
//! it took here, taken through the dispatcher as the Task page takes them; the team run of `team-running`, whose work
//! starts through the dispatcher's `work.start` as the Task page's Run starts it; the team runs the server fenced, as
//! this computer keeps them; the module branch a delivery takes; olha's delivery request that waits for the Admin
//! (04e); and the version the server needs. Fixture data for QA, never shipped.

use std::{collections::HashMap, error::Error, path::Path, process::Command};

use orchester_api::{
  test_support::team_server::{
    AuthorityClaim, AuthorityModule, AuthorityTask, PendingDelivery, Person, Plan, TeamServer,
  },
  v2::{AuthorityRef, Subject, TeamCommandValue},
};
use orchester_app::services::Services;
use orchester_core::{
  id::{ClaimId, ModuleId, OperationId, ProjectId, ServerId, TaskId, WorkId},
  run::CommitId,
  sharing::TaskRevision,
  store::RunStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, TeamSettlement};
use orchester_workspace::actions::{
  Accepted, Action, ActionOutcome, ClaimAction, Dispatcher, Intent, TeamOutcome, WorkAction,
};
use tokio::runtime::Handle;

use super::{State, closed, recorded_start};

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

/// The task of `team-not-ready-ended`: its run on this computer ended, mara's claim on it is stale, and Team delivery,
/// which a Take over starts anew, is not ready here.
const NOT_READY_TASK: u32 = 5;

/// The index of Webhooks v2 among the modules, which KON-12 is in.
const WEBHOOKS: usize = 0;
/// The index of Billing exports among the modules, the person's on this computer.
const BILLING: usize = 1;

/// The base a start names when the checkout has no commit of its own, as in a headless test: fixture data, which the
/// stand-in takes as it takes any commit.
const NO_HEAD: &str = "0000000000000000000000000000000000000000";

/// The oldest desktop the server accepts in `team-version-refused`: the team server's minimum, one minor above this
/// app's own version so the refusal never goes stale when the app is released (`tests/team_qa/team_actions.rs` reads it
/// the same way).
pub fn required_version() -> String {
  let own = env!("CARGO_PKG_VERSION");
  let mut parts = own.split('.');
  let major = parts.next().and_then(|part| part.parse::<u64>().ok());
  let minor = parts.next().and_then(|part| part.parse::<u64>().ok());
  let (Some(major), Some(minor)) = (major, minor) else {
    panic!("the app's version {own} has a major and a minor number")
  };
  format!("{major}.{}.0", minor + 1)
}

/// The delivery of Webhooks v2 olha asked for in `team-admin-work` (04e `GujCo`): how long ago, and the module
/// branch's commit onto the base's.
const OLHA_ASKED: (i64, (&str, &str)) = (
  7,
  (
    "4be19c2a8d1f3e5b7c9a0d2f4e6b8c1a3d5f7e90",
    "91d0a7e3c5b7d9f1a3c5e7b9d1f3a5c7e9b1d3f5",
  ),
);

/// The task mara holds in `team-admin-work`, and how many minutes ago her work last renewed the claim.
const ADMIN_SEES: (u32, i64) = (13, 5);

/// The task olha holds and has blocked in `team-admin-work`, and the one mara holds in `team-claims`, with the reason.
const ADMIN_BLOCKED: u32 = 21;
const KON_16: u32 = 16;
const BLOCKED_REASON: &str = "Waiting for the staging key";

/// The task `team-claims` and `team-admin-work` read as canceled, which nobody holds.
const CANCELED_TASK: u32 = 15;

/// What a team run records of the start that made it: its operation, and the claim and work it reports under.
type Authorized = (OperationId, ClaimId, WorkId);

/// `task` as `state` has it on the server.
pub fn task(state: State, task: &mut AuthorityTask) {
  match state {
    // KON-12 has no work yet: its start here makes it.
    State::TeamRunning if task.number == 12 => task.work = None,
    // KON-5's claim is mara's and stale: Take over starts a new run, which is refused as not ready here.
    State::NotReadyAfterRun if task.number == NOT_READY_TASK => {
      task.claim = claim(MARA, 22, true);
      task.work = Some("paused");
      task.assignee = task.claim.map(|claim| claim.holder);
    }
    // An Admin reads mara's claim on KON-13: Mark as done and Cancel task close it for her (decision D4, T2 `o4K8uq`).
    State::AdminWork if task.number == ADMIN_SEES.0 => {
      task.claim = claim(MARA, ADMIN_SEES.1, false);
      task.work = Some("running");
      task.assignee = task.claim.map(|claim| claim.holder);
    }
    State::AdminWork if task.number == ADMIN_BLOCKED => {
      task.claim = claim(OLHA, 4, false);
      task.work = Some("running");
      task.assignee = task.claim.map(|claim| claim.holder);
      task.blocked_reason = Some(BLOCKED_REASON);
    }
    State::AdminWork if task.number == CANCELED_TASK => task.workflow = "canceled",
    state if closed::is(state) => closed::task(task),
    State::Claims => {
      if let Some((_, claim, work)) = CLAIMS.iter().find(|(number, ..)| *number == task.number) {
        task.claim = *claim;
        task.work = *work;
        task.assignee = claim.map(|claim| claim.holder);
      }
      closed_and_blocked(task);
    }
    _ => {}
  }
}

/// What `team-claims` adds for the lifecycle's states (T11c `dTn0v`, T11f `vyQJx`): KON-16, mara's, is blocked, and
/// KON-15 is canceled; `team-admin-work` has KON-21, olha's, blocked (T11b `x0xP1`) and KON-15 canceled too.
fn closed_and_blocked(task: &mut AuthorityTask) {
  match task.number {
    KON_16 => task.blocked_reason = Some(BLOCKED_REASON),
    CANCELED_TASK => task.workflow = "canceled",
    _ => {}
  }
}

/// `modules` as `state` has them on the server: olha's claim of Webhooks v2 stale, or her request to deliver it waiting
/// for an Admin.
pub fn modules(state: State, modules: &mut [AuthorityModule]) {
  let Some(webhooks) = modules.get_mut(WEBHOOKS) else {
    return;
  };
  match state {
    State::ModuleWork | State::Claims => webhooks.claim = claim(OLHA, 41, true),
    State::AdminWork => {
      let (minutes, commits) = OLHA_ASKED;
      webhooks.pending = Some(PendingDelivery::awaiting_admin(OLHA, minutes, commits));
    }
    _ => {}
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
    State::EditConflict => server.task_changed_first(12, OLHA, "urgent"),
    state if closed::is(state) => closed::records(server),
    _ => {}
  }
}

/// What this computer holds in `state` once Kontur at `root` joined `server` as `id`: the claims it took here, the team
/// runs it started and those the server fenced, and the module branch a delivery takes; `ids` are Kontur's tasks here
/// by number.
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
  let teams = |numbers: &[u32]| as_team(root, (id, project), ids, numbers);
  match state {
    State::MemberRunning | State::ModuleOutage | State::TeamRunning => {
      teams(&[12])?;
      let claim = take(task_subject(ids, 12)?)?;
      if state == State::TeamRunning
        && let Some(task) = local(ids, 12)?
      {
        let authorized = start_here((&dispatcher, runtime), root, (task, claim))?;
        team_run(root, task, authorized, None)?;
      }
    }
    State::Claims => {
      teams(&HELD_HERE)?;
      teams(&FENCED.map(|(number, _)| number))?;
      for number in HELD_HERE {
        take(task_subject(ids, number)?)?;
      }
      server.claim_aged(24, 22, true);
      for (number, fence) in FENCED {
        if let Some(task) = local(ids, number)? {
          // Their claims are someone else's now, or this person's in another session: the ids only name what the
          // fenced runs reported under, and nothing reports under them again.
          let authorized = (OperationId::generate(), ClaimId::generate(), WorkId::generate());
          team_run(root, task, authorized, Some(fence))?;
        }
      }
    }
    State::NotReadyAfterRun => teams(&[NOT_READY_TASK])?,
    // KON-12 is the team's task whose run 8 an archive stopped: the Summary names the resource, as the team's. This
    // computer holds its claim, as the person who ran it does; otherwise the page says another computer has it and
    // Run again has nothing to take (A22 `pUu78` shows no such notice).
    State::TeamArchived => {
      teams(&[12])?;
      take(task_subject(ids, 12)?)?;
    }
    State::ModuleWork | State::AdminWork => {
      take(Subject::Module {
        id: module_id(BILLING)?,
      })?;
      branch(root, "module/billing-exports")?;
    }
    State::StartWaiting | State::StartRunnerRefused | State::StartPastFailure => {
      recorded_start::record(root, (id, project), (state, task_subject(ids, recorded_start::KON_5)?))?;
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
  let ActionOutcome::Team(TeamOutcome::Completed(Accepted::Command(receipt))) = outcome else {
    return Err(format!("the stand-in did not give the claim: {outcome:?}").into());
  };
  match receipt.result {
    TeamCommandValue::Claim { claim, .. } => Ok(claim.id),
    other => Err(format!("the stand-in answered a claim with {other:?}").into()),
  }
}

/// Starts KON-12's work, `task` here, under `claim` through the dispatcher's `work.start`, as the Task page's Run sends
/// it once its choices are made: from the checkout's head, in Webhooks v2 as first read. The stand-in issues the work;
/// its start effect is this computer's run of the task, so the operation is settled completed, as `team_work` settles
/// it once the start's run is recorded.
fn start_here(
  (dispatcher, runtime): (&Dispatcher, &Handle),
  root: &Path,
  (task, claim): (TaskId, ClaimId),
) -> Result<Authorized, Box<dyn Error>> {
  let base_commit = match head(root)? {
    Some(head) => head,
    None => CommitId::new(NO_HEAD)?,
  };
  let intent = Intent::new(Action::Work(WorkAction {
    authority: AuthorityRef {
      subject: Subject::Task { id: task },
      claim_id: claim,
      work_id: None,
    },
    base_commit,
    read: TaskRevision::FIRST,
    module: module_id(WEBHOOKS)?,
    module_read: TaskRevision::FIRST,
  }));
  let outcome = runtime.block_on(dispatcher.dispatch(&intent))?;
  let ActionOutcome::Team(TeamOutcome::AcceptedPending { receipt, .. }) = outcome else {
    return Err(format!("the stand-in did not start the work: {outcome:?}").into());
  };
  let TeamCommandValue::Work { work, .. } = &receipt.result else {
    return Err(format!("the stand-in answered a start with {:?}", receipt.result).into());
  };
  let operation = receipt.operation_id;
  let mut db = ProjectDb::open(root)?;
  let kept = db.team_operation(operation)?.ok_or("the dispatcher kept the start")?;
  let text = kept.receipt.ok_or("the dispatcher kept the start's receipt")?;
  db.settle_team_operation(
    operation,
    TeamSettlement::Completed { receipt: &text },
    Timestamp::now(),
  )?;
  Ok((operation, claim, work.id))
}

/// The id here of the task numbered `number`, when this computer has it.
fn local(ids: &HashMap<u32, String>, number: u32) -> Result<Option<TaskId>, Box<dyn Error>> {
  Ok(ids.get(&number).map(|id| id.parse()).transpose()?)
}

/// Records the tasks numbered `numbers` this computer has as tasks of the team's `project` on `server`, as the first
/// sync records the tasks the server serves by their ids here, before anything acts on them: the dispatcher refuses a
/// team action on a task this computer owns, and the join's sync may not have run yet.
fn as_team(
  root: &Path,
  (server, project): (ServerId, ProjectId),
  ids: &HashMap<u32, String>,
  numbers: &[u32],
) -> Result<(), Box<dyn Error>> {
  let connection = rusqlite::Connection::open(root.join(".orchester").join("orchester.db"))?;
  for &number in numbers {
    let Some(task) = local(ids, number)? else {
      continue;
    };
    connection.execute(
      "UPDATE task_identities SET owner = 'team', server_id = ?2, team_project_id = ?3, team_task_id = id, \
       recorded_revision = (SELECT revision FROM tasks WHERE tasks.id = task_identities.id) \
       WHERE id = ?1 AND owner = 'local'",
      rusqlite::params![task.to_string(), server.to_string(), project.to_string()],
    )?;
  }
  Ok(())
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

/// Records the latest run of `task` as the team run of the start `authorized` names, as a start through the team server
/// leaves it, fenced for `fence` when one is given: the records `team_work` keeps of a start that reached its run. The
/// stand-in's bundles are all the zero hash. A fenced run keeps two reports the server never took, as 03 `hWXxr`
/// counts them: the one it refused with the fence, and the one never sent after it.
fn team_run(
  root: &Path,
  task: TaskId,
  (operation, claim, work): Authorized,
  fence: Option<&str>,
) -> Result<(), Box<dyn Error>> {
  let run = ProjectDb::open(root)?
    .runs_of_task(task)?
    .into_iter()
    .last()
    .ok_or("the seed gave the task a run")?
    .map_err(|_| "the seed's run reads")?;
  let now = Timestamp::now().unix_millis();
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
  let unsent = if fence.is_some() { 2 } else { 0 };
  connection.execute(
    "INSERT INTO team_runs (run_id, operation_id, team_task_id, claim_id, work_id, next_sequence, fence, fenced_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    rusqlite::params![
      run.id().to_string(),
      operation.to_string(),
      task.to_string(),
      claim.to_string(),
      work.to_string(),
      unsent + 1,
      fence,
      fence.map(|_| now)
    ],
  )?;
  if let Some(fence) = fence {
    let refusal = format!(r#"{{"code":"{fence}","details":{{}}}}"#);
    for (sequence, refused) in [(1, Some(refusal.as_str())), (2, None)] {
      connection.execute(
        "INSERT INTO work_report_outbox (event_id, run_id, sequence, operation_id, outcome, request, state, refusal, \
         created_at, settled_at) VALUES (?1, ?2, ?3, ?4, '{}', '{}', ?5, ?6, ?7, ?8)",
        rusqlite::params![
          orchester_core::id::WorkEventId::generate().to_string(),
          run.id().to_string(),
          sequence,
          OperationId::generate().to_string(),
          if refused.is_some() { "refused" } else { "pending" },
          refused,
          now,
          refused.map(|_| now)
        ],
      )?;
    }
  }
  Ok(())
}

/// Makes `branch` at the checkout's head, as an Admin's first assignment of the module named it, when the checkout has
/// a head of its own ([`head`]).
fn branch(root: &Path, branch: &str) -> Result<(), Box<dyn Error>> {
  if head(root)?.is_none() {
    return Ok(());
  }
  let made = git(root).args(["branch", branch]).status()?;
  if !made.success() {
    return Err(format!("git did not make {branch}").into());
  }
  Ok(())
}

/// The commit the checkout at `root` has checked out. Only in a repository whose top is the checkout itself and that
/// has a commit: a headless test's checkout is none, and git would find the repository of a folder above it instead,
/// which is never this scenario's to read or change.
fn head(root: &Path) -> Result<Option<CommitId>, Box<dyn Error>> {
  let top = git(root).args(["rev-parse", "--show-toplevel"]).output()?;
  let top = String::from_utf8_lossy(&top.stdout);
  let ours = !top.trim().is_empty() && Path::new(top.trim()).canonicalize().ok() == root.canonicalize().ok();
  if !ours {
    return Ok(None);
  }
  let head = git(root).args(["rev-parse", "--verify", "--quiet", "HEAD"]).output()?;
  if !head.status.success() {
    return Ok(None);
  }
  Ok(Some(CommitId::new(String::from_utf8_lossy(&head.stdout).trim())?))
}

fn git(root: &Path) -> Command {
  let mut git = Command::new("git");
  git.arg("-C").arg(root);
  git
}
