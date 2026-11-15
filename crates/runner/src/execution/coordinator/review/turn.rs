//! One turn that answers a review question: the implementing step's agent, on the step's connection and model, in the
//! run's worktree, its session resumed; its words and activity kept in the thread as they come, and how it ended kept
//! with the question.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ConnectionError, REASONING_EFFORT, SessionOrigin,
    stream::{AgentEvent, OutputStream},
  },
  id::{ReviewMessageId, RunId},
  orchestrator::{TurnFailure, TurnState},
  permission::NetworkAccess,
  resource::{AgentProfile, CommandEnvironment, ModelId, StepKey},
  review::{AgentWords, QuestionState, ReviewFailure, ReviewMessage, ReviewThread, Said, question_text},
  run::{Run, StepExecutor},
  secret::SecretValue,
  store::{ActivityStore, ConnectionStore, ResourceStore, SettingsStore, WorkspaceStore},
  time::Timestamp,
  workspaces::Holder,
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use tokio::sync::oneshot;

use super::super::chat::Seen;
use crate::{computer::ComputerResources, execution::executor::connection_check, notify::Notifier};

/// Everything a turn starts from.
pub(super) struct ReviewTurn {
  pub(super) root: PathBuf,
  pub(super) notifier: Notifier,
  /// Where the key of the connection's API provider is read from.
  pub(super) computer: Arc<dyn ComputerResources>,
  pub(super) run: Run,
  pub(super) thread: ReviewThread,
  pub(super) question: ReviewMessageId,
  /// The run's implementing step, whose agent answers.
  pub(super) step: StepKey,
  /// How long the answer may run before the runner ends it.
  pub(super) time_limit: Duration,
}

/// What either CLI starts from.
pub(super) struct Prepared {
  pub(super) connection: AgentConnection,
  pub(super) executable: PathBuf,
  pub(super) model: ModelId,
  pub(super) effort: Option<String>,
  /// The API provider's key, read from the keychain for this turn; `None` on the CLI's own sign-in.
  pub(super) key: Option<Arc<SecretValue>>,
  /// The step's session the turn resumes, made on the account it runs on.
  pub(super) session: Option<String>,
  pub(super) worktree: PathBuf,
  pub(super) environment: CommandEnvironment,
  /// The step's own network, for Codex's sandbox.
  pub(super) network: NetworkAccess,
  pub(super) prompt: String,
}

/// What the turn has written so far: its words, and what it does now.
pub(super) struct Answer {
  pub(super) words: ReviewMessageId,
  text: String,
  activity: Option<String>,
  pub(super) seen: Seen,
}

impl ReviewTurn {
  /// Runs the turn to its end, or until `stop` says so, and keeps how it ended. The question was kept answering when
  /// the runner took it.
  pub(super) async fn run(self, stop: oneshot::Receiver<()>) {
    let prepared = match self.prepare() {
      Ok(prepared) => prepared,
      Err(failure) => {
        tracing::warn!(run = %self.run.id(), ?failure, "a review question could not be asked");
        let state = QuestionState::NotAsked { failure };
        self.record(|db| self.settle(db, &Said::Question(QuestionState::Answering), &state));
        return;
      }
    };
    let words = ReviewMessage::agent(String::new(), true, Timestamp::now());
    self.record(|db| {
      db.add_review_message(self.thread.id(), &words)?;
      Ok(true)
    });
    let mut answer = Answer {
      words: words.id(),
      text: String::new(),
      activity: None,
      seen: Seen::default(),
    };
    let ended = if prepared.connection.provider() == AgentProvider::CodexCli {
      self.drive_codex(&prepared, stop, &mut answer).await
    } else {
      self.drive_claude(&prepared, stop, &mut answer).await
    };
    let state = match ended {
      TurnState::Answered => QuestionState::Answered,
      TurnState::Stopped => QuestionState::Stopped,
      TurnState::Failed(failure) => QuestionState::NotAsked {
        failure: ReviewFailure::Turn(failure),
      },
      other => QuestionState::NotAsked {
        failure: ReviewFailure::Turn(TurnFailure::Failed {
          reason: format!("the turn ended {}", other.word()),
        }),
      },
    };
    answer.activity = None;
    self.record(|db| {
      db.write_review_words(answer.words, answer.text.trim(), &answer.said(false))?;
      self.settle(db, &Said::Question(QuestionState::Answering), &state)
    });
  }

  /// Moves the question from `from` to `to`; `false` when it no longer said `from`.
  fn settle(&self, db: &mut ProjectDb, from: &Said, to: &QuestionState) -> Result<bool, orchester_store::StoreError> {
    db.change_review_message(self.question, |said| (said == from).then(|| Said::Question(to.clone())))
  }

  /// What the turn starts from, or why it cannot start.
  fn prepare(&self) -> Result<Prepared, ReviewFailure> {
    let cannot = |reason: String| ReviewFailure::Turn(TurnFailure::CannotStart { reason });
    let db = ProjectDb::open(&self.root).map_err(|error| cannot(error.to_string()))?;
    let Some(StepExecutor::Agent { profile, model, .. }) =
      self.run.snapshot().step(&self.step).map(|step| step.executor())
    else {
      return Err(ReviewFailure::NoImplementingStep);
    };
    let body: AgentProfile = db
      .version::<AgentProfile>(profile.resource, profile.number)
      .map_err(|error| cannot(error.to_string()))?
      .ok_or_else(|| cannot(format!("agent profile version {} is missing", profile.number)))?
      .body()
      .clone();
    let chosen = db
      .step_connection(self.run.id(), &self.step)
      .map_err(|error| cannot(error.to_string()))?;
    let runs_on = match chosen {
      Some(chosen) => chosen,
      None => db
        .connection_here(profile.resource, body.connection())
        .map_err(|error| cannot(error.to_string()))?,
    };
    let stored = db
      .connection(runs_on)
      .map_err(|error| cannot(error.to_string()))?
      .ok_or(ReviewFailure::Turn(TurnFailure::ConnectionGone))?;
    let connection = AgentConnection::try_from(stored).map_err(|error| match error {
      ConnectionError::NotAgent(_) => ReviewFailure::Turn(TurnFailure::ConnectionGone),
      other => cannot(format!("cannot read the step's connection: {other}")),
    })?;
    if connection.provider().is_native() {
      return Err(ReviewFailure::NativeAgent);
    }
    if let Some(problem) = connection.account().start_problem(Path::is_dir) {
      return Err(ReviewFailure::Turn(TurnFailure::Account { problem }));
    }
    let executable = connection
      .executable()
      .filter(|_| connection.provider().runs_agents())
      .ok_or_else(|| cannot("the connection has no CLI program".to_owned()))?
      .to_path_buf();
    let key = connection
      .api_provider()
      .map(|on| connection_check::api_key(self.computer.as_ref(), db.project().id(), on.key()))
      .transpose()
      .map_err(cannot)?;
    let worktree = worktree(&db, &self.root, self.run.id())
      .ok_or_else(|| cannot("the run's worktree is not there any more".to_owned()))?;
    let network = self
      .run
      .step_permissions(&self.step, body.permissions())
      .map_or(NetworkAccess::Off, |permissions| permissions.network().clone());
    let effort = body
      .settings()
      .iter()
      .find_map(|(name, value)| (name == REASONING_EFFORT).then(|| value.to_string()));
    let question = self
      .thread
      .message(self.question)
      .ok_or_else(|| cannot("the question is not in its thread".to_owned()))?;
    Ok(Prepared {
      session: self
        .session(&db, &connection)
        .map_err(|error| cannot(error.to_string()))?,
      environment: db.project_environment().map_err(|error| cannot(error.to_string()))?,
      prompt: question_text(&self.thread, question),
      connection,
      executable,
      model: model.clone(),
      effort,
      key,
      worktree,
      network,
    })
  }

  /// The session the step's agent last reported, when it was made where `connection` runs now: a session lives in its
  /// account's folder.
  fn session(
    &self,
    db: &ProjectDb,
    connection: &AgentConnection,
  ) -> Result<Option<String>, orchester_store::StoreError> {
    let executions: Vec<_> = self
      .run
      .attempts()
      .iter()
      .flat_map(|attempt| attempt.executions())
      .filter(|execution| execution.step() == &self.step)
      .map(|execution| execution.id())
      .collect();
    let invocations = db.invocations(self.run.id())?;
    let Some(last) = invocations
      .iter()
      .rev()
      .find(|invocation| executions.contains(&invocation.execution) && invocation.session.is_some())
    else {
      return Ok(None);
    };
    let account = db
      .invocation_accounts(self.run.id())?
      .into_iter()
      .find(|(invocation, _)| *invocation == last.id)
      .map(|(_, account)| account);
    let made = SessionOrigin {
      account,
      provider: db.invocation_provider(last.id)?,
    };
    Ok(
      made
        .continues_on(&SessionOrigin::of(connection))
        .then(|| last.session.clone())
        .flatten(),
    )
  }

  /// Keeps what `event` of the CLI says: the words, what it does now, or how the turn goes.
  pub(super) fn keep(&self, event: AgentEvent, answer: &mut Answer) {
    match event {
      AgentEvent::Message { text } if !text.trim().is_empty() => {
        if !answer.text.is_empty() {
          answer.text.push_str("\n\n");
        }
        answer.text.push_str(text.trim());
        self.write(answer);
      }
      AgentEvent::CommandStarted { command, .. } => {
        answer.activity = Some(command);
        self.write(answer);
      }
      AgentEvent::TurnCompleted { .. } => answer.seen.answered = true,
      AgentEvent::NotSignedIn { .. } => answer.seen.failure = Some(TurnFailure::NotSignedIn),
      AgentEvent::TurnFailed { message } | AgentEvent::Error { message } => {
        answer
          .seen
          .failure
          .get_or_insert(TurnFailure::Failed { reason: message });
      }
      AgentEvent::Raw {
        stream: OutputStream::Stderr,
        line,
      } if !line.trim().is_empty() => answer.seen.stderr = Some(line),
      _ => {}
    }
  }

  /// Keeps the words written so far, as they stream in.
  fn write(&self, answer: &Answer) {
    self.record(|db| {
      db.write_review_words(answer.words, &answer.text, &answer.said(true))?;
      Ok(true)
    });
  }

  /// Applies `change` to the project's database and tells the desktop the run's review changed; a change that cannot
  /// be kept is logged, and the turn goes on.
  fn record(&self, change: impl FnOnce(&mut ProjectDb) -> Result<bool, orchester_store::StoreError>) {
    let kept = ProjectDb::open(&self.root).and_then(|mut db| change(&mut db));
    if let Err(error) = kept {
      tracing::error!(run = %self.run.id(), %error, "cannot keep what a review answer did");
    }
    self.notifier.send(Event::RunChanged {
      run: self.run.id(),
      task: self.run.task(),
    });
  }
}

impl Answer {
  /// What the words say now.
  fn said(&self, writing: bool) -> Said {
    Said::Agent(AgentWords {
      writing,
      activity: self.activity.clone(),
    })
  }
}

/// The folder `run`'s work is in, in the repository at `root`: its own worktree, else the workspace it holds, else the
/// one it worked in last while no other run took it since; `None` when it has none any more.
fn worktree(db: &ProjectDb, root: &Path, run: RunId) -> Option<PathBuf> {
  let own = root
    .join(orchester_core::integration::WORKTREES_DIR)
    .join(run.to_string());
  if own.is_dir() {
    return Some(own);
  }
  let records = db
    .workspaces()
    .inspect_err(|error| tracing::warn!(%run, %error, "the workspaces cannot be read"))
    .ok()?;
  let held = records
    .iter()
    .find(|record| record.holder == Some(Holder::Run { run }))
    .or_else(|| {
      records
        .iter()
        .find(|record| record.holder.is_none() && record.last_run == Some(run))
    })?;
  Some(held.number.path(root)).filter(|path| path.is_dir())
}
