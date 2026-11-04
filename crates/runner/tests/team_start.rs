//! A team start end to end into this runner (team authority P4): the desktop's `TeamWork` sends `work.start` to a fake
//! team server with the server's routes and shapes (the workspace's `tests/team_work`, shared here by `#[path]`),
//! reads the bundle and its file by hash, keeps them as the run's accepted inputs, and asks this runner's scheduler to
//! start the run, which executes from them. A crash or a lost answer at each boundary (the work's answer, the bundle
//! file read, the runner not there, the runner's answer lost) is recovered once: one work, one run. The finished run
//! reports `succeeded`, which keeps the claim and completes nothing. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test team_start team_start`.

mod support;

#[path = "../../workspace/tests/team_work/fixture.rs"]
mod fixture;
#[path = "../../workspace/tests/actions/http.rs"]
mod http;
#[path = "../../workspace/tests/team_work/team_commands.rs"]
mod team_commands;
#[path = "../../workspace/tests/team_work/team_server.rs"]
mod team_server;

use std::{fs, path::Path, sync::Arc};

use orchester_api::{
  client::{Client, PlainHttp, ServerAddress},
  v2::{AuthorityRef, ExecutionState, Subject, TeamCommandValue},
};
use orchester_core::{
  id::{ClaimId, OperationId, RunId},
  run::{
    CommitId, RunStatus,
    inputs::{LocalBindings, LocalConsent, ProfileBinding},
  },
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::TaskRevision,
  store::{ConnectionStore, RunStore},
};
use orchester_runner::execution::Scheduler;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  actions::{Accepted, Action, ActionOutcome, ClaimAction, Dispatcher, Intent, TeamOutcome, WorkAction},
  runner::{RequestError, RunRequestError},
  secrets::Vault,
  sharing::Servers,
  team_reads::TeamSession,
  team_work::{StartChoices, StartOutcome, TeamRunner, TeamStartRequest, TeamWork},
  workspace::Workspace,
};
use parking_lot::Mutex;
use tokio::sync::watch;

use self::{
  fixture::{Config, NOTES, SKILL_SLUG, TASK_NUMBER},
  support::{Setup, git, wait_for, wait_for_idle},
  team_server::{MEMBER, TeamServer},
};

/// No secret is read or kept here.
struct NoVault;

impl SecretVault for NoVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Failed("no secrets in these tests".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// What the runner's next start requests meet before it answers them itself.
#[derive(Clone, Copy)]
enum Hiccup {
  /// The runner is not there.
  Unavailable,
  /// The runner starts the run and its answer is lost.
  StartsThenLost,
}

/// This runner's scheduler, asked as the desktop asks the runner process.
#[derive(Clone)]
struct SchedulerRunner {
  scheduler: Scheduler,
  hiccups: Arc<Mutex<Vec<Hiccup>>>,
}

impl TeamRunner for SchedulerRunner {
  fn start_accepted_run(&self, operation: OperationId) -> impl Future<Output = Result<RunId, RunRequestError>> + Send {
    let runner = self.clone();
    async move {
      let hiccup = {
        let mut hiccups = runner.hiccups.lock();
        (!hiccups.is_empty()).then(|| hiccups.remove(0))
      };
      if let Some(Hiccup::Unavailable) = hiccup {
        return Err(RunRequestError::Request(RequestError::NotConnected));
      }
      let started = runner.scheduler.start_accepted_run(operation).await;
      match (hiccup, started) {
        (Some(Hiccup::StartsThenLost), _) => Err(RunRequestError::Request(RequestError::Lost)),
        (_, started) => started.map_err(RunRequestError::Rejected),
      }
    }
  }
}

/// Links the project at `root` to `server`'s project, as a member.
fn link(root: &Path, server: &TeamServer) {
  rusqlite::Connection::open(root.join(".orchester").join("orchester.db"))
    .expect("open")
    .execute(
      "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
       last_synced_at) VALUES (1, ?1, ?2, 'ORC', 'member', 1, 0, 1)",
      [server.server.to_string(), server.project.to_string()],
    )
    .expect("link the project");
}

fn client(server: &TeamServer) -> Client {
  Client::new(
    ServerAddress::parse(&server.url).expect("address"),
    PlainHttp::NotAllowed,
  )
  .expect("client")
}

/// The dispatcher of the person's session on `server` for the project at `root`, with the application data in `data`.
fn dispatcher(root: &Path, data: &Path, server: &TeamServer) -> (Dispatcher, watch::Sender<u64>) {
  let workspace = Workspace::new(AppDb::open(&data.join("orchester.db")).expect("database"));
  let servers = Servers::new(workspace, Vault::with(Arc::new(NoVault)));
  let generation = watch::channel(0).0;
  let token = SecretValue::new(MEMBER.to_owned()).expect("token");
  let session = TeamSession::new(
    (client(server), token),
    (server.server, server.project),
    generation.subscribe(),
  );
  (Dispatcher::new(root.to_path_buf(), servers, Some(session)), generation)
}

async fn claim(dispatcher: &Dispatcher, server: &TeamServer) -> ClaimId {
  let acquire = Action::Claim(ClaimAction::Acquire {
    subject: Subject::Task { id: server.task },
  });
  let outcome = dispatcher.dispatch(&Intent::new(acquire)).await.expect("dispatched");
  let ActionOutcome::Team(TeamOutcome::Completed(Accepted::Command(receipt))) = outcome else {
    panic!("not claimed: {outcome:?}");
  };
  let TeamCommandValue::Claim { claim, .. } = receipt.result else {
    panic!("not a claim: {receipt:?}");
  };
  claim.id
}

/// The start from `base`, the profile bound to the project's Codex connection.
fn request(root: &Path, server: &TeamServer, (claim, base): (ClaimId, &str), config: &Config) -> TeamStartRequest {
  let connection = ProjectDb::open(root).expect("open").connections().expect("connections")[0].id();
  TeamStartRequest {
    operation: OperationId::generate(),
    work: WorkAction {
      authority: AuthorityRef {
        subject: Subject::Task { id: server.task },
        claim_id: claim,
        work_id: None,
      },
      base_commit: CommitId::new(base).expect("commit"),
      read: TaskRevision::FIRST,
      module: server.module,
      module_read: TaskRevision::FIRST,
    },
    choices: StartChoices {
      bindings: LocalBindings {
        profiles: vec![ProfileBinding {
          resource: config.profile,
          connection,
          account: None,
        }],
        references: Vec::new(),
      },
      consent: LocalConsent::default(),
    },
  }
}

fn waiting(outcome: &StartOutcome) -> bool {
  matches!(outcome, StartOutcome::Waiting(_) | StartOutcome::RunnerUnavailable(_))
}

#[tokio::test(flavor = "multi_thread")]
async fn team_start_runs_in_the_runner_and_recovers_once_at_every_boundary() {
  let project = support::project(&Setup::new("Append one line."));
  let root = project.repo.path();
  let data = tempfile::tempdir().expect("application data");
  let head = git(root, &["rev-parse", "HEAD"]);
  let config = Config::new("Change README.md for {{task.id}}.");
  let server = TeamServer::start(config.clone());
  link(root, &server);
  let (claiming, _claim_generation) = dispatcher(root, data.path(), &server);
  let claim = claim(&claiming, &server).await;
  let request = request(root, &server, (claim, &head), &config);
  let (scheduler, mut events) = support::scheduler(root);
  let runner = SchedulerRunner {
    scheduler: scheduler.clone(),
    hiccups: Arc::new(Mutex::new(vec![Hiccup::Unavailable, Hiccup::StartsThenLost])),
  };
  let (dispatcher, _generation) = dispatcher(root, data.path(), &server);
  let work = TeamWork::new(dispatcher, runner);

  // The work's answer is lost, then the bundle's file read is cut off, then the runner is not there, then it starts
  // the run and its answer is lost: each recovery goes on from where the start stopped.
  server.lose_next_response();
  assert!(waiting(&work.start(request).await.expect("started")));
  server.drop_next_file();
  for _ in 0..3 {
    let (_, outcome) = work.recover().await.expect("recovered").remove(0);
    let outcome = outcome.expect("carried on");
    assert!(waiting(&outcome), "{outcome:?}");
  }
  let (_, started) = work.recover().await.expect("recovered").remove(0);
  let started = started.expect("carried on");
  let StartOutcome::Started(run) = started else {
    panic!("not started: {started:?}");
  };
  assert!(work.recover().await.expect("recovered").is_empty());

  let done = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  wait_for_idle(&scheduler).await;
  // The branch takes the bundle's task number, and the skill's folder its label's slug.
  assert_eq!(done.base().as_str(), head);
  assert_eq!(done.branch().as_str(), format!("orchester/orc-{TASK_NUMBER}-run-1"));
  let execution = done.current_attempt().expect("an attempt").executions()[0].id();
  let placed = root
    .join(".orchester/skills")
    .join(execution.to_string())
    .join(SKILL_SLUG)
    .join("references/notes.md");
  assert_eq!(fs::read(placed).expect("the skill's file, from the kept bundle"), NOTES);

  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.runs_of_task(done.task()).expect("runs").len(), 1, "one run");
  assert_eq!(server.state().works_made, 1, "one work");

  // Reported: an execution success keeps the claim and completes nothing.
  let reported = work.report().await.expect("reported");
  assert_eq!((reported.queued, reported.acknowledged), (1, 1));
  let state = work.run_state(run).expect("read").expect("a team run");
  assert_eq!(state.local, RunStatus::Succeeded);
  assert_eq!(state.accepted.map(|work| work.state), Some(ExecutionState::Succeeded));
  assert!(server.state().claim.is_some(), "the claim is kept");
}
