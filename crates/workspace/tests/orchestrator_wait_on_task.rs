//! The chat's `wait_on_task` and the end of a run's wait on a task (ORC-766, `orchestrator::waits_on`): the tool keeps
//! the relation and its row at once in any turn, and refuses the run's own task, another project's, a run that does
//! not wait for a message or a decision and a second wait; the waited task's merge into the run's base branch is an
//! event in the conversation that leads the waiting task, which starts a turn there, and so is the waited task
//! canceled or closed without that merge; the person answering the run ends the wait without one.
//! `cargo test -p orchester-workspace --test orchestrator_wait_on_task wait_on_task`.

mod support;

use std::{collections::HashSet, sync::Arc};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::{AgentMessage, Message, MessageBody},
  id::{ChatId, ChatMessageId, ConnectionId, RunId, TaskId},
  integration::{MergeKind, RunMerge},
  name::Name,
  orchestrator::{
    ActionOutcome, AgentWait, ChatAction, EntryKind, OrchestratorSettings, Subscription, TaskEvent, TaskEventKind,
    TurnState, WaitKind, WaitTurn, WaitsOnRow, WatchTarget,
  },
  resource::ModelId,
  revision::Revision,
  run::{Answerer, BranchName, Run, WaitReason},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::{ActivityStore, ConnectionStore, ConversationStore, MergeStore},
  task::organization::{ChangeActor, WorkflowStatus},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb, RunWaitEnd};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, ToolError, Translate},
  orchestrator::{
    chat::ChatView,
    tools::{self, ChatScope},
    waits::{Followed, Switches, check_merges_in, follow_in},
    waits_on::{
      dropping::drop_wait,
      view::{Unmet, WaitState},
    },
  },
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{conversation::follow::Sent, detail::TaskDetail, integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::{OpenProject, Workspace},
};
use serde_json::{Value, json};
use support::{Fixture, at};
use tempfile::TempDir;
use tokio::runtime::Runtime;

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

/// The project of [`Fixture`], open in the app's session, with the conversation that leads its tasks.
struct Desktop {
  fixture: Fixture,
  services: app_mcp::Services,
  runners: Runners,
  servers: Servers,
  open: OpenProject,
  runtime: Runtime,
  scope: ChatScope,
  _data: TempDir,
}

impl Desktop {
  fn new() -> Self {
    let mut fixture = Fixture::new();
    ready(&mut fixture);
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    let open = workspace.open(fixture.repo.path()).expect("register the project");
    let runtime = Runtime::new().expect("runtime");
    let runners = Runners::new(
      fixture.repo.path().join("no-runner"),
      runtime.handle().clone(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    );
    let version = orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version");
    let servers = Servers::new(workspace.clone(), Vault::with(Arc::new(NoVault)), version);
    let services = app_mcp::Services::new(
      workspace,
      runners.clone(),
      servers.clone(),
      Programs::processes(),
      (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
    );
    let chat = talking(&mut fixture, "Lead the delivery work", 10);
    let scope = ChatScope {
      project: fixture.db.project().id(),
      name: "Kontur".to_owned(),
      root: fixture.repo.path().to_path_buf(),
      chat,
      on_its_own: false,
    };
    Self {
      fixture,
      services,
      runners,
      servers,
      open,
      runtime,
      scope,
      _data: data,
    }
  }

  /// How `run`'s wait stands, as the conversation's *waits on* row reads it.
  fn standing(&self) -> Vec<(RunId, WaitState)> {
    let shown = ChatView::load_in(self.fixture.repo.path(), self.scope.chat)
      .expect("read")
      .expect("kept");
    let mut waits: Vec<(RunId, WaitState)> = shown.waits_on.values().map(|wait| (wait.run, wait.state)).collect();
    waits.sort_by_key(|(run, _)| run.to_string());
    waits
  }

  /// *Drop the wait* of `run`, pressed by `by`.
  fn drop_wait(&self, run: RunId, by: Answerer) -> Result<Option<Sent>, orchester_workspace::messages::Message> {
    self
      .runtime
      .block_on(drop_wait((&self.runners, &self.servers), &self.open, run, by))
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.fixture.repo.path()).expect("project database")
  }

  /// `wait_on_task` for run `run` and task `task`, in `scope`, as the chat's agent calls it.
  fn wait_on(&self, scope: &ChatScope, run: RunId, task: &str) -> Result<Value, ToolError> {
    let Value::Object(arguments) = json!({ "project": "Kontur", "run": run.to_string(), "task": task }) else {
      unreachable!("arguments are an object");
    };
    let translate = Translate::new(|message| message.key.to_owned());
    self.runtime.block_on(tools::call(
      (&self.services, &translate, &self.runners),
      scope,
      "wait_on_task",
      arguments,
    ))
  }

  /// A task titled `title` whose run waits for an answer to its agent's question, from the base branch `main`.
  fn waiting(&mut self, title: &str) -> (TaskId, Run) {
    let task = self.fixture.task(title, at(40));
    let mut run = self.fixture.started_run_from(task, (&"a".repeat(40), "main"), at(50));
    ask(&mut self.fixture, &mut run);
    (task, run)
  }

  /// The run of `task`, which succeeded, merged into `branch`.
  fn merged_into(&mut self, task: TaskId, branch: &str) -> RunId {
    let mut run = self.fixture.started_run_from(task, (&"a".repeat(40), branch), at(60));
    self.fixture.succeed(&mut run, at(61));
    let merge = RunMerge {
      run: run.id(),
      into: BranchName::new(branch).expect("branch"),
      revision: Revision::new(&"b".repeat(40)).expect("revision"),
      kind: MergeKind::MergeCommit,
      merged_at: Timestamp::now(),
    };
    self.fixture.db.save_run_merge(&merge).expect("merged");
    run.id()
  }

  fn last_action(&self) -> ChatAction {
    self
      .db()
      .chat_entries(self.scope.chat)
      .expect("entries")
      .into_iter()
      .rev()
      .find_map(|entry| match entry.kind {
        EntryKind::Action(action) => Some(action),
        _ => None,
      })
      .expect("an action is kept")
  }

  /// The events of the conversation, with their turns.
  fn events(&self) -> Vec<(TaskEvent, Option<WaitTurn>)> {
    self
      .db()
      .chat_entries(self.scope.chat)
      .expect("entries")
      .into_iter()
      .filter_map(|entry| match entry.kind {
        EntryKind::Wait {
          wait: AgentWait {
            kind: WaitKind::Event(event),
            ..
          },
          turn,
        } => Some((event, turn)),
        _ => None,
      })
      .collect()
  }

  /// How `run`'s latest wait on a task ended; `None` while it waits.
  fn ended(&self, run: RunId) -> Option<RunWaitEnd> {
    let kept = self
      .db()
      .run_wait(run)
      .expect("read")
      .expect("the run waited on a task");
    kept.ended.map(|ended| ended.end)
  }

  /// Follows the waits with the project's runner attached, *Let the Orchestrator answer agents* on when `answers`.
  fn follow(&self, answers: bool) -> Followed {
    let (root, programs) = (self.fixture.repo.path(), Programs::processes());
    let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
    let switches = Switches { drives: answers };
    follow_in((root, self.scope.project, &programs), &checks, (switches, true)).expect("followed")
  }
}

/// A checked, signed-in Claude Code connection, published as what the Orchestrator runs on.
fn ready(fixture: &mut Fixture) {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · Kontur").expect("name"),
    AgentProvider::ClaudeCodeCli,
    fixture.repo.path().join("claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute program");
  let haiku = CatalogModel::new(ModelId::new("haiku").expect("model"), Vec::new());
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, vec![haiku]),
      },
    })
    .expect("check");
  fixture.db.create_connection(&connection).expect("connection");
  let settings = OrchestratorSettings::new(connection.id(), ModelId::new("haiku").expect("model"), None);
  fixture
    .db
    .publish_orchestrator(&settings, Timestamp::now())
    .expect("published");
}

/// `run`'s agent asks "Keep email sign-in?" in the conversation, and its step waits for the answer.
fn ask(fixture: &mut Fixture, run: &mut Run) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(55))
    .expect("invocation")
    .id;
  let question = Message::from_agent(
    run.task(),
    run.id(),
    (execution, invocation),
    AgentMessage::Question,
    MessageBody::new("Keep email sign-in?").expect("text"),
    at(58),
  );
  fixture.db.add_agent_message(&question).expect("asked");
  let events = run.wait_step(execution, WaitReason::Message, at(60)).expect("waits");
  fixture.record(run, &events);
}

/// A conversation the person talked in at `at(start)`, whose answered turn did nothing to the agents.
fn talking(fixture: &mut Fixture, title: &str, start: i64) -> ChatId {
  let chat = orchester_core::orchestrator::Chat::named(ChatId::generate(), title, at(start));
  let person = ChatMessageId::generate();
  let text = MessageBody::new(title).expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(start))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(start + 1))
    .expect("ended");
  chat.id()
}

/// The refusal's code.
fn code(refused: &ToolError) -> String {
  refused.to_json(|message| message.key.to_owned())["error"]["code"]
    .as_str()
    .expect("a code")
    .to_owned()
}

#[test]
fn wait_on_task_keeps_the_relation_and_its_row_at_once_in_any_turn() {
  let mut desktop = Desktop::new();
  let (_, run) = desktop.waiting("Retry failed deliveries");
  let limiter = desktop.fixture.task("Shared rate limiter", at(41));
  let told = desktop.wait_on(&desktop.scope, run.id(), "orc-2").expect("waits");
  assert_eq!(told["wait"]["task"], "ORC-1", "{told}");
  assert_eq!(told["wait"]["waits_on"], "ORC-2", "{told}");
  assert_eq!(told["wait"]["base_branch"], "main", "{told}");
  let kept = desktop.db().run_wait(run.id()).expect("read").expect("kept");
  assert_eq!((kept.wait.waited(), kept.ended), (limiter, None));
  let action = desktop.last_action();
  assert_eq!(action.outcome, ActionOutcome::Done);
  assert_eq!(action.task.as_deref(), Some("ORC-1"));
  assert_eq!(
    action.waits_on,
    Some(WaitsOnRow {
      run: run.id(),
      number: 1,
      waited: "ORC-2".to_owned()
    })
  );

  // In a turn Orchester started by itself it is done at once too.
  let (_, other) = desktop.waiting("Card form");
  let on_its_own = ChatScope {
    on_its_own: true,
    ..desktop.scope.clone()
  };
  desktop.wait_on(&on_its_own, other.id(), "ORC-2").expect("waits");
  assert_eq!(desktop.ended(other.id()), None, "it waits");
}

#[test]
fn wait_on_task_refuses_its_own_task_another_projects_a_run_that_does_not_wait_and_a_second_wait() {
  let mut desktop = Desktop::new();
  let (_, run) = desktop.waiting("Retry failed deliveries");
  desktop.fixture.task("Shared rate limiter", at(41));
  desktop.fixture.task("Receipts", at(42));
  let refused = |task: &str| code(&desktop.wait_on(&desktop.scope, run.id(), task).expect_err("refused"));
  assert_eq!(refused("ORC-1"), "own_task");
  assert_eq!(refused("ABC-2"), "other_project");
  let action = desktop.last_action();
  assert!(matches!(&action.outcome, ActionOutcome::Refused { code, .. } if code == "other_project"));
  assert_eq!(
    action.task.as_deref(),
    Some("ORC-1"),
    "the refusal names the run's task"
  );
  assert_eq!(action.waits_on, None);

  desktop.wait_on(&desktop.scope, run.id(), "ORC-2").expect("waits");
  assert_eq!(refused("ORC-3"), "already_waits", "one task at a time");

  let working = desktop.fixture.task("Card form", at(43));
  let working = desktop
    .fixture
    .started_run_from(working, (&"a".repeat(40), "main"), at(50));
  let refused = desktop
    .wait_on(&desktop.scope, working.id(), "ORC-2")
    .expect_err("refused");
  assert_eq!(code(&refused), "not_waiting_for_person", "its agent works");
}

#[test]
fn wait_on_task_merged_into_the_runs_base_starts_a_turn_in_the_leading_conversation() {
  let mut desktop = Desktop::new();
  let (task, run) = desktop.waiting("Retry failed deliveries");
  let limiter = desktop.fixture.task("Shared rate limiter", at(41));
  // Another conversation is the newer one; the subscription leads the task here.
  talking(&mut desktop.fixture, "Anything else?", 20);
  let watch = Subscription::new(desktop.scope.chat, WatchTarget::Task(task), "", at(21));
  desktop.fixture.db.start_subscription(&watch).expect("watching");
  desktop.wait_on(&desktop.scope, run.id(), "ORC-2").expect("waits");
  desktop.follow(false);
  assert!(desktop.events().is_empty(), "nothing happened to the waited task");

  let merged = desktop.merged_into(limiter, "main");
  let turn = desktop.follow(true).turn.expect("the event starts a turn by itself");
  assert_eq!(turn.chat, desktop.scope.chat, "in the leading conversation");
  let events = desktop.events();
  assert_eq!(events.len(), 1, "{events:?}");
  assert_eq!(
    events[0].0.kind,
    TaskEventKind::WaitedMerged {
      task: limiter,
      run: merged
    }
  );
  assert_eq!(events[0].1.as_ref().map(|started| started.message), Some(turn.message));
  assert_eq!(desktop.ended(run.id()), Some(RunWaitEnd::Merged));
  assert_eq!(desktop.follow(true), Followed::default(), "one event, one turn");
}

#[test]
fn wait_on_task_canceled_or_closed_without_the_merge_tells_the_conversation_and_ends_the_wait() {
  let mut desktop = Desktop::new();
  let (_, canceled_run) = desktop.waiting("Retry failed deliveries");
  let (_, elsewhere_run) = desktop.waiting("Card form");
  let canceled = desktop.fixture.task("Shared rate limiter", at(41));
  let elsewhere = desktop.fixture.task("Receipts", at(42));
  desktop
    .wait_on(&desktop.scope, canceled_run.id(), "ORC-3")
    .expect("waits");
  desktop
    .wait_on(&desktop.scope, elsewhere_run.id(), "ORC-4")
    .expect("waits");
  desktop
    .fixture
    .db
    .close_task_organization(canceled, WorkflowStatus::Canceled, (ChangeActor::Person, at(70)))
    .expect("canceled");
  // Merged into another branch: the task is done without the merge the run waits for.
  desktop.merged_into(elsewhere, "release");
  assert!(desktop.follow(false).kept);
  let kinds: Vec<TaskEventKind> = desktop.events().into_iter().map(|(event, _)| event.kind).collect();
  assert_eq!(kinds.len(), 2, "{kinds:?}");
  assert!(kinds.contains(&TaskEventKind::WaitedClosed {
    task: canceled,
    run: None
  }));
  assert!(
    kinds
      .iter()
      .any(|kind| matches!(kind, TaskEventKind::WaitedClosed { task, run: Some(_) } if *task == elsewhere))
  );
  assert_eq!(desktop.ended(canceled_run.id()), Some(RunWaitEnd::Canceled));
  assert_eq!(desktop.ended(elsewhere_run.id()), Some(RunWaitEnd::ClosedUnmerged));
}

#[test]
fn wait_on_task_answered_by_the_person_ends_the_wait_without_an_event() {
  let mut desktop = Desktop::new();
  let (_, mut run) = desktop.waiting("Retry failed deliveries");
  let limiter = desktop.fixture.task("Shared rate limiter", at(41));
  desktop.wait_on(&desktop.scope, run.id(), "ORC-2").expect("waits");
  // The person answers on the run's own wait card: its step goes on.
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.resume_step(execution, Timestamp::now()).expect("goes on");
  desktop.fixture.record(&run, &events);
  desktop.follow(true);
  assert_eq!(desktop.ended(run.id()), Some(RunWaitEnd::Answered));
  desktop.merged_into(limiter, "main");
  desktop.follow(true);
  assert!(desktop.events().is_empty(), "the wait had ended");
}

/// ORC-784: the row and the task page read how the wait stands: it waits, then the waited task is canceled and nothing
/// will end it but the person, who drops it; the run's agent is told, and a second drop is refused.
#[test]
fn wait_on_task_stands_until_the_person_drops_a_wait_that_cannot_be_met() {
  let mut desktop = Desktop::new();
  let (task, run) = desktop.waiting("Retry failed deliveries");
  let limiter = desktop.fixture.task("Shared rate limiter", at(41));
  desktop.wait_on(&desktop.scope, run.id(), "ORC-2").expect("waits");
  assert_eq!(desktop.standing(), vec![(run.id(), WaitState::Waiting)]);
  let page = TaskDetail::load_in(desktop.fixture.repo.path(), task).expect("read");
  let wait = page.waits_on.expect("the task page reads the wait");
  assert_eq!(
    (
      &*wait.task,
      &*wait.waited,
      &*wait.waited_title,
      &*wait.base,
      wait.number
    ),
    ("ORC-1", "ORC-2", "Shared rate limiter", "main", 1)
  );

  desktop
    .fixture
    .db
    .close_task_organization(limiter, WorkflowStatus::Canceled, (ChangeActor::Person, at(70)))
    .expect("canceled");
  desktop.follow(false);
  let standing = desktop.standing();
  assert!(
    matches!(
      standing[..],
      [(
        _,
        WaitState::Unmet {
          how: Unmet::Canceled,
          ..
        }
      )]
    ),
    "{standing:?}"
  );
  assert!(standing[0].1.holds(), "the run still waits for the person");

  let before = desktop.db().messages(task).expect("messages").len();
  let automated = desktop
    .drop_wait(run.id(), Answerer::Automation)
    .expect_err("only the person drops a wait");
  assert_eq!(automated.key, "orchestrator.waitsOn.onlyYou");
  assert_ne!(
    desktop.ended(run.id()),
    Some(RunWaitEnd::Dropped),
    "the wait still holds"
  );
  assert!(desktop.standing()[0].1.holds(), "the run still waits for the person");
  assert_eq!(desktop.db().messages(task).expect("messages").len(), before);

  let sent = desktop.drop_wait(run.id(), Answerer::Person).expect("dropped");
  assert!(sent.is_some(), "the waiting agent is told");
  assert_eq!(desktop.ended(run.id()), Some(RunWaitEnd::Dropped));
  assert!(matches!(desktop.standing()[..], [(_, WaitState::Dropped { .. })]));
  let told = desktop.db().messages(task).expect("messages");
  let told = told.last().expect("a message to the agent");
  assert_eq!(told.run(), Some(run.id()));
  assert!(told.body().as_str().contains("ORC-2"), "{told:?}");
  let refused = desktop.drop_wait(run.id(), Answerer::Person).expect_err("dropped once");
  assert_eq!(refused.key, "orchestrator.waitsOn.dropGone");
}
