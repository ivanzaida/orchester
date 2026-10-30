//! The QA project Kontur (key KON, as in `data/orchester2.pen`): a new git repository with one commit, its
//! project database, the project registered in an application database, and the data of one
//! [`Scenario`] written through the store's and core's APIs, the way the app and the runner write it.
//! Shared by the `seed` example and its tests (`tests/seed.rs`).

mod catalogue;
mod files;
mod git;
mod run;
pub mod scenarios;
mod shapes;

use std::{
  collections::BTreeMap,
  fs, io,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  connection::ConnectionError,
  id::{CloudAccountId, IdempotencyKey, ProjectId, RunId, TagId, TaskId},
  integration::MergeError,
  limits::ProjectLimits,
  name::Name,
  project::{Project, ProjectKey},
  revision::Revision,
  run::{RunError, SnapshotError},
  store::{ActivityStore, SettingsStore, TaskStore},
  task::{
    QueueError, Task, TaskDetails, TaskLimits,
    organization::{BlockingTask, ChangeActor, DependencyGate, Priority, TaskOrganization, WorkflowStatus},
  },
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::{AppDb, ProjectDb, RunStoreError, StoreError, location};

pub use self::scenarios::Scenario;
use self::{
  catalogue::{Catalogue, Published},
  git::GitError,
};

/// The design's project.
const PROJECT_NAME: &str = "Kontur";
const PROJECT_KEY: &str = "KON";
/// The program the application database belongs to: the database lies beside it.
const APP_PROGRAM: &str = "orchester-app";

/// What to seed, and where.
pub struct SeedRequest {
  /// The folder of the app's program, which keeps its `orchester.db` beside it.
  pub data_dir: PathBuf,
  /// A folder that does not exist yet, or an empty one: the repository is made there.
  pub repo: PathBuf,
  pub scenario: Scenario,
  /// `orchester-fake-codex`, which the project's Codex connection runs; `live` needs it.
  pub fake_codex: Option<PathBuf>,
  /// When the scenario's data was last touched; its runs started minutes to days before.
  pub now: Timestamp,
}

/// What was seeded.
pub struct Seeded {
  pub project: ProjectId,
  pub repo: PathBuf,
  pub app_database: PathBuf,
  /// The scenario's tasks, in key order, with the state each was left in.
  pub tasks: Vec<SeededTask>,
}

pub struct SeededTask {
  pub key: String,
  pub title: String,
  pub state: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum SeedError {
  #[error("{0} exists and is not empty; the seed only makes new repositories")]
  RepoNotEmpty(PathBuf),
  #[error("{0} exists; the seed only registers projects in a new application database")]
  AppDatabaseExists(PathBuf),
  #[error("the live scenario runs `orchester-fake-codex`: pass --fake-codex <path>")]
  FakeCodexRequired,
  #[error("{0} is not a program file")]
  FakeCodexMissing(PathBuf),
  #[error("cannot use {path}: {source}")]
  Io {
    path: PathBuf,
    #[source]
    source: io::Error,
  },
  #[error(transparent)]
  Git(#[from] GitError),
  #[error("cannot merge the run: {0}")]
  Merge(#[from] MergeError),
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  RunStore(#[from] RunStoreError),
  #[error("the run refused the transition: {0}")]
  Run(#[from] RunError),
  #[error("cannot pin the pipeline: {0}")]
  Snapshot(#[from] SnapshotError),
  #[error("invalid connection: {0}")]
  Connection(#[from] ConnectionError),
  #[error("cannot queue the task: {0}")]
  Queue(#[from] QueueError),
  #[error("cannot edit the project database directly: {0}")]
  Sqlite(#[from] rusqlite::Error),
  #[error("the pipeline has no step {0}")]
  NoSuchStep(String),
  #[error("the step's agent is not at work")]
  NotAtWork,
  #[error("the run's worktree was removed")]
  WorktreeRemoved,
  #[error("the run has no token budget to stop at")]
  NoBudget,
  #[error("the task names no pipeline to run")]
  NoPipeline,
  #[error("the task changed while it was saved")]
  TaskChanged,
  #[error("no task {0} to add to")]
  NoSuchTask(String),
  #[error("invalid seed data: {0}")]
  Invalid(String),
}

/// The project being seeded.
pub struct Kontur {
  db: ProjectDb,
  repo: PathBuf,
  /// The repository's first commit on `main`, which runs start from.
  base: Revision,
  now: Timestamp,
  fake_codex: Option<PathBuf>,
  app_database: PathBuf,
  /// The account Kontur is registered for, when the scenario gives it to one.
  owner: Option<CloudAccountId>,
  tags: BTreeMap<&'static str, TagId>,
  seeded: Vec<SeededTask>,
}

/// The name `text`, a literal of the seed.
fn name(text: &str) -> Name {
  Name::new(text).expect("the seed's names are valid")
}

/// Every task's limits: generous, so only the scenarios that stop at a limit reach one.
fn limits() -> TaskLimits {
  let budget = TokenBudget::new(500_000).expect("above zero");
  TaskLimits::new(Some(budget), Some(Duration::from_secs(90 * 60))).expect("a time limit above zero")
}

impl Kontur {
  /// The design's catalogue ([`catalogue::kontur`]), published a week before now.
  fn catalogue(&mut self) -> Result<Catalogue, SeedError> {
    let published = self.ago(7 * 24 * 60);
    let connection = catalogue::codex(&mut self.db, self.fake_codex.as_deref(), published)?;
    catalogue::kontur(&mut self.db, connection, published)
  }

  /// The design's catalogue with Feature delivery ending in a Deliver step ([`catalogue::delivering`]), published a
  /// week before now.
  fn delivering_catalogue(&mut self) -> Result<Catalogue, SeedError> {
    let published = self.ago(7 * 24 * 60);
    let connection = catalogue::codex(&mut self.db, self.fake_codex.as_deref(), published)?;
    catalogue::delivering(&mut self.db, connection, published)
  }

  /// The Usage page's catalogue ([`catalogue::usage`]) on a Codex and a Claude Code connection that are
  /// recorded, never run, published two weeks before now.
  fn usage_catalogue(&mut self) -> Result<Catalogue, SeedError> {
    let published = self.ago(14 * 24 * 60);
    // Programs that are never there: the runs are recorded, not run.
    let programs = self.repo.join(".orchester").join("recorded");
    let codex = catalogue::recorded_connection(
      &mut self.db,
      (
        "Codex · Kontur",
        orchester_core::connection::AgentProvider::CodexCli,
        &programs.join("codex"),
      ),
      published,
    )?;
    let claude = catalogue::recorded_connection(
      &mut self.db,
      (
        "Claude Code · Kontur",
        orchester_core::connection::AgentProvider::ClaudeCodeCli,
        &programs.join("claude"),
      ),
      published,
    )?;
    catalogue::usage(&mut self.db, (codex, claude), published)
  }

  /// The catalogue of live runs ([`catalogue::live`]), published an hour before now.
  fn live_catalogue(&mut self) -> Result<Catalogue, SeedError> {
    let published = self.ago(60);
    let connection = catalogue::codex(&mut self.db, self.fake_codex.as_deref(), published)?;
    catalogue::live(&mut self.db, connection, published)
  }

  /// `minutes` before the seed's now.
  fn ago(&self, minutes: i64) -> Timestamp {
    Timestamp::from_unix_millis(self.now.unix_millis() - minutes * 60_000)
  }

  /// `minutes` before the seed's now, but no earlier than a minute into today (local time): what must be
  /// today's stays today's when the seed runs just after midnight. Then what follows it may be stamped
  /// a little after now.
  fn today_ago(&self, minutes: i64) -> Timestamp {
    let first = self.now.local_day_start().unix_millis() + 60_000;
    Timestamp::from_unix_millis(self.ago(minutes).unix_millis().max(first))
  }

  /// A draft titled `title`; without `pipeline` and with an empty `brief` it misses details.
  fn task(
    &mut self,
    (title, brief, criteria): (&str, &str, &[&str]),
    pipeline: Option<&Published>,
    at: Timestamp,
  ) -> Result<Task, SeedError> {
    let details = TaskDetails {
      title: name(title),
      brief: brief.to_owned(),
      acceptance_criteria: criteria.iter().map(|criterion| (*criterion).to_owned()).collect(),
      pipeline: pipeline.map(Published::id),
      limits: limits(),
    };
    let mut created = self.db.create_tasks(IdempotencyKey::generate(), vec![details], at)?;
    Ok(created.remove(0))
  }

  /// Queues the draft `task` on the published `pipeline`, as Run does before its first run.
  fn queue(&mut self, task: &mut Task, pipeline: &Published) -> Result<(), SeedError> {
    task.queue(pipeline.snapshot().pipeline().number)?;
    *task = self.db.save_task(task)?.saved().ok_or(SeedError::TaskChanged)?;
    Ok(())
  }

  /// The project's tag `title`, made on first use.
  fn tag(&mut self, title: &'static str) -> Result<TagId, SeedError> {
    if let Some(id) = self.tags.get(title) {
      return Ok(*id);
    }
    let id = self.db.create_project_tag(name(title))?.id();
    self.tags.insert(title, id);
    Ok(id)
  }

  /// Sets how `task` is organized, as a person sets it: workflow status, priority, tags, and the task
  /// whose merge it waits for.
  fn organize(
    &mut self,
    task: TaskId,
    (priority, status): (Priority, WorkflowStatus),
    tags: &[&'static str],
    blocked_by: Option<TaskId>,
  ) -> Result<(), SeedError> {
    let tags = tags.iter().map(|tag| self.tag(tag)).collect::<Result<Vec<_>, _>>()?;
    let dependencies = blocked_by
      .map(|task| BlockingTask {
        task,
        gate: DependencyGate::Merge,
      })
      .into_iter()
      .collect();
    let organization = TaskOrganization::new(status, priority, None, dependencies, tags)
      .map_err(|error| SeedError::Invalid(error.to_string()))?;
    self
      .db
      .set_task_organization(task, &organization, ChangeActor::Person, self.now)?;
    Ok(())
  }

  /// Notes the state `task` was left in, for the seed's report.
  fn seeded(&mut self, task: &Task, state: &'static str) {
    self.seeded.push(SeededTask {
      key: task.key().to_string(),
      title: task.details().title.as_str().to_owned(),
      state,
    });
  }

  /// Damages the stored record of `run` the way an edit outside Orchester would: it cannot be read back.
  fn damage(&self, run: RunId) -> Result<(), SeedError> {
    let changed = rusqlite::Connection::open(self.database())?
      .execute("UPDATE runs SET snapshot = 'not json' WHERE id = ?1", [run.to_string()])?;
    if changed != 1 {
      return Err(SeedError::Invalid(format!("damaging run {run} changed {changed} rows")));
    }
    Ok(())
  }

  /// Sets the project's daily budget to what its runs used today, so the budget is used up.
  fn spend_daily_budget(&mut self) -> Result<(), SeedError> {
    let used = self.db.tokens_used_since(self.now.local_day_start())?;
    let budget = TokenBudget::new(used.get())
      .map_err(|_| SeedError::Invalid("no run used tokens today to spend the daily budget with".to_owned()))?;
    let limits = self.db.project_limits()?;
    let spent = ProjectLimits::new(limits.default_run_budget(), budget, limits.default_time_limit())
      .map_err(|error| SeedError::Invalid(error.to_string()))?
      .with_agent_slots(limits.agent_slots());
    self.db.save_project_limits(&spent)?;
    Ok(())
  }

  fn database(&self) -> PathBuf {
    self.repo.join(".orchester").join("orchester.db")
  }

  /// Leaves `file` beside the application database, which the QA app started on the scenario looks for
  /// ([`scenarios::qa_app_asks`]).
  fn ask_qa_app(&self, file: &str) -> Result<(), SeedError> {
    let path = self.app_database.with_file_name(file);
    fs::write(&path, "").map_err(io_error(&path))
  }

  /// The task with key `key`, e.g. `KON-12`.
  fn find(&self, key: &str) -> Result<Task, SeedError> {
    self
      .db
      .tasks()?
      .into_iter()
      .find(|task| task.key().to_string() == key)
      .ok_or_else(|| SeedError::NoSuchTask(key.to_owned()))
  }
}

/// Seeds `request.scenario`: a new repository with the project, registered in the application database
/// of `request.data_dir`.
pub fn seed(request: &SeedRequest) -> Result<Seeded, SeedError> {
  let fake_codex = request.fake_codex.as_deref().map(program).transpose()?;
  if request.scenario.needs_fake_codex() && fake_codex.is_none() {
    return Err(SeedError::FakeCodexRequired);
  }
  let data_dir = absolute(&request.data_dir)?;
  let app_database = location::app_database_beside(&data_dir.join(APP_PROGRAM));
  // Never another app's database: a mistaken data folder must not migrate or change an install's.
  if app_database.try_exists().map_err(io_error(&app_database))? {
    return Err(SeedError::AppDatabaseExists(app_database));
  }
  let repo = absolute(&request.repo)?;
  new_folder(&repo)?;
  fs::create_dir_all(&data_dir).map_err(io_error(&data_dir))?;
  let base = git::init(&repo, &files::initial())?;
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new(PROJECT_KEY).expect("a valid project key"),
    name(PROJECT_NAME),
  );
  let mut kontur = Kontur {
    db: ProjectDb::create(&repo, project.clone())?,
    repo: repo.clone(),
    base,
    now: request.now,
    fake_codex,
    app_database,
    owner: None,
    tags: BTreeMap::new(),
    seeded: Vec::new(),
  };
  request.scenario.seed(&mut kontur)?;
  let Kontur {
    app_database,
    seeded,
    owner,
    ..
  } = kontur;
  let mut db = AppDb::open(&app_database)?;
  // The account the scenario gave it to or the signed-in account's project, otherwise this computer's.
  let owner = match owner {
    Some(owner) => Some(owner),
    None => db.cloud_account()?.map(|account| account.account),
  };
  db.register_project(&project, &repo, request.now, owner.as_ref())?;
  Ok(Seeded {
    project: project.id(),
    repo,
    app_database,
    tasks: seeded,
  })
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> SeedError + '_ {
  move |source| SeedError::Io {
    path: path.to_path_buf(),
    source,
  }
}

fn absolute(path: &Path) -> Result<PathBuf, SeedError> {
  std::path::absolute(path).map_err(io_error(path))
}

/// Makes `repo`, which must not exist or be empty: the seed never writes into a folder that holds anything.
fn new_folder(repo: &Path) -> Result<(), SeedError> {
  match fs::read_dir(repo) {
    Ok(entries) => {
      return match entries.count() {
        0 => Ok(()),
        _ => Err(SeedError::RepoNotEmpty(repo.to_path_buf())),
      };
    }
    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
    Err(error) => return Err(io_error(repo)(error)),
  }
  fs::create_dir_all(repo).map_err(io_error(repo))
}

/// The program at `path`, made absolute: a connection stores an absolute program path.
fn program(path: &Path) -> Result<PathBuf, SeedError> {
  let program = absolute(path)?;
  if !program.is_file() {
    return Err(SeedError::FakeCodexMissing(program));
  }
  Ok(program)
}
