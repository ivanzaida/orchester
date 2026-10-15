//! What the QA window shows: the seed's `tasks` project with a Claude Code connection to `orchester-fake-claude`
//! (`Claude · Kontur`, its catalog Opus 5.5 and Sonnet 5.5) beside the seed's Codex one (`Codex · Kontur`, to
//! `orchester-fake-codex`, which the Orchestrator does not run on yet), the Orchestrator set to run on the Claude one,
//! and conversations whose last turns ended in each way the chat shows (`conversations`). Every folder is under the QA
//! folder.

use std::{error::Error, path::PathBuf};

use orchester_core::{
  account::AccountCli,
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ConnectionId, ProjectId},
  name::Name,
  orchestrator::{
    ActionOutcome, Chat, ChatAction, OrchestratorSettings, TurnFailure, TurnState,
    proposal::{ProposedStart, StartBasis, brief_excerpt, brief_is_cut},
  },
  resource::{ModelId, Pipeline, ResourceKind},
  store::{ConnectionStore, ResourceStore, TaskStore},
  task::{Task, TaskStatus},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::kontur;

/// The model the Orchestrator runs with, as the connection's catalog lists it.
const MODEL: &str = "claude-opus-5-5";

/// The seeded data the window opens, and the conversations by what their last turn shows.
pub struct Prepared {
  pub database: PathBuf,
  /// The project's repository.
  pub repo: PathBuf,
  pub project: ProjectId,
  pub conversations: Conversations,
  /// Replies with links in the app, to another project, to the web and to addresses the chat does not follow.
  pub links: ChatId,
  /// What the start prints: the scenario and the task the answered conversation created, with its state.
  pub shown: String,
}

pub struct Conversations {
  /// Answered turns with replies and two actions, one done and one refused.
  pub answered: ChatId,
  pub stopped: ChatId,
  /// The CLI was not signed in.
  pub failed: ChatId,
  pub interrupted: ChatId,
  /// A start the agent proposed, of a task edited since: *Start* is refused as changed since proposed.
  pub proposal_changed: ChatId,
  /// Publish, cancel and merge proposals, waiting and settled ([`super::changes`]).
  pub changes: ChatId,
}

/// What the QA folder `root` is seeded with: `claude` the fake CLI the Claude connection runs, `codex` the one the
/// seed's Codex connection runs; `orchestrator` whether the Orchestrator is set to run on the Claude one, and
/// `claude_connection` whether the project has that connection at all.
pub fn prepare(
  root: &std::path::Path,
  (claude, codex): (PathBuf, PathBuf),
  (orchestrator, claude_connection): (bool, bool),
) -> Result<Prepared, Box<dyn Error>> {
  let scenario = kontur::scenarios::named("tasks").ok_or("the seed has the tasks scenario")?;
  let seeded = kontur::seed(&kontur::SeedRequest {
    data_dir: root.join("app"),
    repo: root.join("repo-kontur"),
    scenario,
    fake_codex: Some(codex),
    now: Timestamp::now(),
  })?;
  let mut db = ProjectDb::open(&seeded.repo)?;
  let connection = connection(claude)?;
  if claude_connection {
    db.create_connection(&connection)?;
  }
  if orchestrator {
    let settings = OrchestratorSettings::new(connection.id(), ModelId::new(MODEL)?, Some("high".to_owned()));
    db.publish_orchestrator(&settings, Timestamp::now())?;
  }
  let keys: Vec<String> = seeded.tasks.iter().map(|task| task.key.clone()).collect();
  let shown = seeded.tasks.first().map_or_else(String::new, |task| {
    format!("{} · {} {} ({})", scenario.name(), task.key, task.title, task.state)
  });
  let conversations = conversations(&mut db, (&keys, connection.id()))?;
  let atlas = super::links::register_atlas(&seeded.app_database, root)?;
  let first = seeded.tasks.first().ok_or("the tasks scenario has tasks")?;
  let links = super::links::conversation(
    &mut db,
    (seeded.project, atlas),
    (&first.key, &first.title),
    connection.id(),
  )?;
  Ok(Prepared {
    database: seeded.app_database,
    repo: seeded.repo,
    project: seeded.project,
    conversations,
    links,
    shown,
  })
}

/// `Claude · Kontur`, checked and signed in, running `program`.
fn connection(program: PathBuf) -> Result<AgentConnection, Box<dyn Error>> {
  let now = Timestamp::now();
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · Kontur")?,
    AgentProvider::ClaudeCodeCli,
    program,
    now,
  )?;
  let model = |id: &str| -> Result<CatalogModel, Box<dyn Error>> {
    Ok(CatalogModel::new(
      ModelId::new(id)?,
      vec!["medium".to_owned(), "high".to_owned()],
    ))
  };
  connection.record_check(ConnectionCheck {
    at: now,
    result: CheckResult::Available {
      version: "stand-in".to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(
        CatalogSource::Provider,
        vec![model(MODEL)?, model("claude-sonnet-5-5")?],
      ),
    },
  })?;
  Ok(connection)
}

/// A minute `n` minutes ago.
pub(super) fn ago(minutes: i64) -> Timestamp {
  Timestamp::from_unix_millis(Timestamp::now().unix_millis() - minutes * 60_000)
}

/// One turn of `chat`: the person's `text` at `at`, the answer's `parts`, and how it ended.
pub(super) struct Turn<'a> {
  pub(super) text: &'a str,
  pub(super) at: Timestamp,
  pub(super) parts: Vec<Part<'a>>,
  pub(super) end: TurnState,
}

pub(super) enum Part<'a> {
  Reply(&'a str),
  Action(Box<ChatAction>),
}

pub(super) fn keep(
  db: &mut ProjectDb,
  chat: ChatId,
  connection: ConnectionId,
  turns: Vec<Turn<'_>>,
) -> Result<ChatId, Box<dyn Error>> {
  let model = ModelId::new(MODEL)?;
  for turn in turns {
    let body = MessageBody::new(turn.text)?;
    let message = ChatMessageId::generate();
    let started = db.chat(chat)?.unwrap_or_else(|| Chat::start(chat, &body, turn.at));
    db.keep_chat_message(&started, (message, &body), turn.at)?;
    let account = AccountCli::ClaudeCode.default_account();
    db.set_chat_session(chat, &format!("qa-{chat}"), (connection, account))?;
    for (offset, part) in (1_i64..).zip(turn.parts) {
      let at = Timestamp::from_unix_millis(turn.at.unix_millis() + offset * 20_000);
      match part {
        Part::Reply(text) => db.add_chat_reply(chat, (text, &model), at)?,
        Part::Action(action) => db.add_chat_action(chat, &action, at)?,
      }
    }
    // It ended a minute after it was sent.
    let ended = Timestamp::from_unix_millis(turn.at.unix_millis() + 60_000);
    db.end_chat_turn(message, &turn.end, ended)?;
  }
  Ok(chat)
}

/// The agent created `first`, and could not start `second`.
fn actions(first: &str, second: &str) -> (ChatAction, ChatAction) {
  let created = ChatAction::new("create_task", Some(first.to_owned()), ActionOutcome::Done);
  let refused = ChatAction::new(
    "run_task",
    Some(second.to_owned()),
    ActionOutcome::Refused {
      code: "task_not_ready".to_owned(),
      reason: "Needs a pipeline".to_owned(),
    },
  );
  (created, refused)
}

fn conversations(
  db: &mut ProjectDb,
  (keys, connection): (&[String], ConnectionId),
) -> Result<Conversations, Box<dyn Error>> {
  let first = keys.first().cloned().unwrap_or_default();
  let second = keys.get(1).cloned().unwrap_or_default();
  let (created, refused) = actions(&first, &second);
  let reply = format!(
    "I created **{first}** as a draft with its brief and acceptance criteria. Nothing is queued and no agent has \
     started.\n\nI read the task list and the project's pipelines; I didn't read repository files."
  );
  let refusal = format!("I can't start **{second}** yet: it has no pipeline. Choose one in Tasks, then ask me again.");
  let answered = keep(
    db,
    ChatId::generate(),
    connection,
    vec![
      Turn {
        text: "Draft a task for the delivery log on the endpoint page.",
        at: ago(30),
        parts: vec![Part::Reply(&reply), Part::Action(Box::new(created))],
        end: TurnState::Answered,
      },
      Turn {
        text: &format!("Start {second} now."),
        at: ago(24),
        parts: vec![Part::Reply(&refusal), Part::Action(Box::new(refused))],
        end: TurnState::Answered,
      },
    ],
  )?;
  let stopped = keep(
    db,
    ChatId::generate(),
    connection,
    vec![Turn {
      text: "Plan the webhook delivery work. Break it into tasks we can review.",
      at: ago(26 * 60),
      parts: vec![Part::Reply(
        "I'm drafting tasks from the delivery work. So far: 01 Record every delivery attempt, 02 Delivery log on endpoint page",
      )],
      end: TurnState::Stopped,
    }],
  )?;
  let failed = keep(
    db,
    ChatId::generate(),
    connection,
    vec![Turn {
      text: "What's running, what's waiting and what's blocked right now?",
      at: ago(50),
      parts: Vec::new(),
      end: TurnState::Failed(TurnFailure::NotSignedIn),
    }],
  )?;
  let proposal_changed = changed_proposal(db, connection)?;
  let changes = super::changes::conversation(db, connection)?;
  let interrupted = keep(
    db,
    ChatId::generate(),
    connection,
    vec![Turn {
      text: "Suggest an order for the Todo tasks and which can run in parallel.",
      at: ago(9 * 24 * 60),
      parts: vec![Part::Reply("Let me look at the queue.")],
      end: TurnState::Failed(TurnFailure::Interrupted),
    }],
  )?;
  Ok(Conversations {
    answered,
    stopped,
    failed,
    interrupted,
    proposal_changed,
    changes,
  })
}

/// The agent proposed starting the first draft with a pipeline and a brief, and the brief was edited after, as a person
/// editing it in Tasks would: *Start* no longer runs what was proposed and is refused.
fn changed_proposal(db: &mut ProjectDb, connection: ConnectionId) -> Result<ChatId, Box<dyn Error>> {
  let task = db
    .tasks()?
    .into_iter()
    .find(|task| {
      let details = task.details();
      task.status() == TaskStatus::Draft && details.pipeline.is_some() && !details.brief.is_empty()
    })
    .ok_or("the tasks scenario has a draft with a pipeline and a brief")?;
  let start = proposed(db, &task)?;
  let key = task.key().to_string();
  let reply = format!(
    "**{key}** can run now on {}. Start it below when you're ready.",
    start.pipeline
  );
  let action = ChatAction::new("run_task", Some(key.clone()), ActionOutcome::Proposed(Box::new(start)));
  let chat = keep(
    db,
    ChatId::generate(),
    connection,
    vec![Turn {
      text: &format!("Start {key}."),
      at: ago(12),
      parts: vec![Part::Reply(&reply), Part::Action(Box::new(action))],
      end: TurnState::Answered,
    }],
  )?;
  let mut details = task.details().clone();
  details.brief.push_str(" Keep the last 50 attempts per endpoint.");
  db.update_task(task.id(), task.revision(), details)?
    .saved()
    .ok_or("nothing else changed the task")?;
  Ok(chat)
}

/// The start of draft `task` as the agent proposes it: on its pipeline's latest published version.
fn proposed(db: &ProjectDb, task: &Task) -> Result<ProposedStart, Box<dyn Error>> {
  let details = task.details();
  let pipeline = details.pipeline.ok_or("the draft has a pipeline")?;
  let name = db
    .resources(ResourceKind::Pipeline)?
    .into_iter()
    .find(|resource| resource.id() == pipeline)
    .ok_or("the draft's pipeline is in the catalogue")?
    .name()
    .as_str()
    .to_owned();
  let version = db
    .latest_version::<Pipeline>(pipeline)?
    .map(|version| version.number().get());
  Ok(ProposedStart {
    pipeline: name,
    title: details.title.as_str().to_owned(),
    brief: brief_excerpt(&details.brief),
    brief_cut: brief_is_cut(&details.brief),
    criteria: details.acceptance_criteria.clone(),
    basis: StartBasis {
      revision: task.revision().get(),
      pipeline: Some(pipeline),
      version,
    },
  })
}
