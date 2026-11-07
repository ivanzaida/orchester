//! Team work through the server (`orchester_workspace::team_work`, team authority P4) against a fake team server on
//! loopback with the server's routes and shapes ([`team_server`], [`team_commands`], [`team_deliveries`]): a start
//! recovers once from a crash or an outage at each boundary and never makes a second work or run ([`start`]); work
//! reports wait in order across an outage and a restart, with the outcomes as decided and local success apart from
//! server acceptance ([`reports`]); a takeover fences the run and keeps its output here; the heartbeat renews only
//! running work ([`heartbeat`]); and a module's delivery merges only on the receipt's effect and reports it once,
//! across lost answers, outages and a crash ([`delivery`], P4c), and recovers from its edge cases
//! ([`delivery_recovery`]). Runs are recorded by a stand-in for the runner that keeps them as the runner does; the
//! runner's own tests start the real one (`crates/runner/tests/team_start.rs`).
//!
//! `cargo test -p orchester-workspace --test team_work team_work_`

#[path = "../actions/http.rs"]
mod http;

mod bundle_schema;
mod completion;
mod delivery;
mod delivery_recovery;
mod fixture;
mod heartbeat;
mod here;
mod reports;
mod start;
mod team_commands;
mod team_deliveries;
mod team_server;

use std::{
  collections::VecDeque,
  fs,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_api::{
  client::{Client, PlainHttp, ServerAddress},
  v2::{AuthorityRef, Subject, TeamCommandValue},
};
use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::{ClaimId, ConnectionId, OperationId, ProjectId, RunId},
  integration::base::RunBaseSource,
  name::Name,
  project::{Project, ProjectKey},
  revision::Revision,
  run::{
    BranchName, CommitId, NewRun, Run, RunError, RunEvent,
    inputs::{InputsOrigin, LocalBindings, LocalConsent, ProfileBinding},
  },
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::TaskRevision,
  store::{ConnectionStore, RunStore},
  time::Timestamp,
};
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
use tempfile::TempDir;
use tokio::sync::watch;

use self::{
  fixture::Config,
  team_server::{MEMBER, OTHER, TeamServer},
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

/// A team project in a folder of its own, linked to the fake server's project, with a Codex connection.
struct Checkout {
  dir: TempDir,
  servers: Servers,
  connection: ConnectionId,
  _data: TempDir,
}

impl Checkout {
  fn linked(server: &TeamServer) -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/team-work-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("project-")
      .tempdir_in(&scratch)
      .expect("project");
    fs::create_dir(dir.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      Name::new("Kontur").expect("name"),
    );
    let mut db = ProjectDb::create(dir.path(), project).expect("create project");
    let codex = AgentConnection::new(
      ConnectionId::generate(),
      Name::new("Codex").expect("name"),
      AgentProvider::CodexCli,
      dir.path().join("codex.exe"),
      Timestamp::now(),
    )
    .expect("connection");
    db.create_connection(&codex).expect("a connection");
    rusqlite::Connection::open(dir.path().join(".orchester/orchester.db"))
      .expect("open")
      .execute(
        "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
         last_synced_at) VALUES (1, ?1, ?2, 'KON', 'member', 1, 0, 1)",
        [server.server.to_string(), server.project.to_string()],
      )
      .expect("link the project");
    let data = tempfile::Builder::new()
      .prefix("data-")
      .tempdir_in(&scratch)
      .expect("data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    Self {
      dir,
      servers: Servers::new(
        workspace,
        Vault::with(Arc::new(NoVault)),
        orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version"),
      ),
      connection: codex.id(),
      _data: data,
    }
  }

  fn root(&self) -> &Path {
    self.dir.path()
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.root()).expect("open")
  }

  /// The dispatcher of `token`'s session on `server`, and the session's generation it keeps.
  fn dispatcher(&self, server: &TeamServer, token: &str) -> (Dispatcher, watch::Sender<u64>) {
    let token = SecretValue::new(token.to_owned()).expect("token");
    let generation = watch::channel(0).0;
    let session = TeamSession::new(
      (client(server), token),
      (server.server, server.project),
      generation.subscribe(),
    );
    let dispatcher = Dispatcher::new(self.root().to_path_buf(), self.servers.clone(), Some(session));
    (dispatcher, generation)
  }

  /// The team work of `token`'s session, its runs started by `runner`.
  fn work(&self, server: &TeamServer, token: &str, runner: StoreRunner) -> (TeamWork<StoreRunner>, watch::Sender<u64>) {
    let (dispatcher, generation) = self.dispatcher(server, token);
    (TeamWork::new(dispatcher, runner), generation)
  }

  /// Claims the server's task in `token`'s session; the claim.
  async fn claim(&self, server: &TeamServer, token: &str) -> ClaimId {
    let (dispatcher, _generation) = self.dispatcher(server, token);
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

  /// The start of the task's work under `claim`, the profile bound to this project's Codex connection.
  fn start_request(&self, server: &TeamServer, claim: ClaimId, config: &Config) -> TeamStartRequest {
    TeamStartRequest {
      operation: OperationId::generate(),
      work: WorkAction {
        authority: AuthorityRef {
          subject: Subject::Task { id: server.task },
          claim_id: claim,
          work_id: None,
        },
        base_commit: CommitId::new(&"a".repeat(40)).expect("commit"),
        read: TaskRevision::FIRST,
        module: server.module,
        module_read: TaskRevision::FIRST,
      },
      choices: StartChoices {
        bindings: LocalBindings {
          profiles: vec![ProfileBinding {
            resource: config.profile,
            connection: self.connection,
            account: None,
            required_cli: None,
            mcp_servers: Vec::new(),
          }],
          references: Vec::new(),
        },
        consent: LocalConsent::default(),
      },
    }
  }

  /// Applies `change` to run `run` as the runner would record it.
  fn change_run(&self, run: RunId, change: impl FnOnce(&mut Run, Timestamp) -> Result<Vec<RunEvent>, RunError>) {
    let mut db = self.db();
    let mut current = db.run(run).expect("read").expect("the run");
    let events = change(&mut current, Timestamp::now()).expect("a transition");
    db.append_events(&current, &events).expect("recorded");
  }
}

fn client(server: &TeamServer) -> Client {
  Client::new(
    ServerAddress::parse(&server.url).expect("address"),
    PlainHttp::NotAllowed,
  )
  .expect("client")
}

/// The task claimed in the person's session and its work started: the team work, the run and the session's generation.
async fn started(checkout: &Checkout, server: &TeamServer) -> (TeamWork<StoreRunner>, RunId, watch::Sender<u64>) {
  let claim = checkout.claim(server, MEMBER).await;
  let config = server.state().config.clone();
  let request = checkout.start_request(server, claim, &config);
  let (work, generation) = checkout.work(server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  let outcome = work.start(request).await.expect("started");
  let StartOutcome::Started(run) = outcome else {
    panic!("not started: {outcome:?}");
  };
  (work, run, generation)
}

/// Another member takes the task's claim `observed` over, in their own session.
async fn take_over(server: &TeamServer, observed: ClaimId) {
  let other = Checkout::linked(server);
  let (dispatcher, _generation) = other.dispatcher(server, OTHER);
  let take_over = Action::Claim(ClaimAction::TakeOver {
    subject: Subject::Task { id: server.task },
    observed,
  });
  let outcome = dispatcher.dispatch(&Intent::new(take_over)).await.expect("dispatched");
  assert!(
    matches!(outcome, ActionOutcome::Team(TeamOutcome::Completed(_))),
    "{outcome:?}"
  );
}

/// What the stand-in runner does with its next start requests, in order; then it starts runs.
#[derive(Clone, Copy, Debug)]
enum Hiccup {
  /// The runner is not there: nothing is started.
  Unavailable,
  /// The runner starts the run and its answer is lost.
  StartsThenLost,
}

/// Records and starts the run of accepted inputs as the runner does, by the operation's one run key, without
/// executing it.
#[derive(Clone)]
struct StoreRunner {
  root: PathBuf,
  hiccups: Arc<Mutex<VecDeque<Hiccup>>>,
  asked: Arc<Mutex<usize>>,
}

impl StoreRunner {
  fn new(root: &Path, hiccups: &[Hiccup]) -> Self {
    Self {
      root: root.to_path_buf(),
      hiccups: Arc::new(Mutex::new(hiccups.iter().copied().collect())),
      asked: Arc::new(Mutex::new(0)),
    }
  }

  fn asked(&self) -> usize {
    *self.asked.lock()
  }

  fn record(&self, operation: OperationId) -> RunId {
    let mut db = ProjectDb::open(&self.root).expect("open");
    let accepted = db
      .accepted_team_inputs(operation)
      .expect("read")
      .expect("the accepted inputs");
    let InputsOrigin::Team(acceptance) = accepted.inputs.bundle().origin().clone() else {
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
    if run.status() == &orchester_core::run::RunStatus::Pending {
      let events = run.start(Timestamp::now()).expect("started");
      db.append_events(&run, &events).expect("recorded");
    }
    run.id()
  }
}

impl TeamRunner for StoreRunner {
  fn start_accepted_run(&self, operation: OperationId) -> impl Future<Output = Result<RunId, RunRequestError>> + Send {
    let runner = self.clone();
    async move {
      *runner.asked.lock() += 1;
      let hiccup = runner.hiccups.lock().pop_front();
      match hiccup {
        Some(Hiccup::Unavailable) => Err(RunRequestError::Request(RequestError::NotConnected)),
        Some(Hiccup::StartsThenLost) => {
          runner.record(operation);
          Err(RunRequestError::Request(RequestError::Lost))
        }
        None => Ok(runner.record(operation)),
      }
    }
  }
}
