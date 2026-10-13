//! What the QA window shows: the seed's `accounts` project (its Codex connection on Default, a run stopped at
//! Default's plan limit, a running run) and accounts of both CLIs in each state Settings › Accounts names
//! ([`super::accounts`]), with three more connections on them: one whose account is signed out. The running run
//! started an agent on Work, so Work can't be removed; Work and Client have plan limits a run reported. Every folder
//! is under the QA folder: the CLIs' own folders too.

use std::{error::Error, fs, path::PathBuf, time::Duration};

use orchester_core::{
  account::AgentAccount,
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionAccount, ConnectionCheck, ModelCatalog,
    SignIn,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits},
  },
  id::{ConnectionId, ProjectId, RunId},
  name::Name,
  run::RunStatus,
  store::{ActivityStore, ConnectionStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{accounts, kontur, stand_ins};

/// The scenario the window's project is seeded with, and the states its tasks are in.
const SCENARIO: &str = "accounts";
const PLAN_LIMIT_STATE: &str = "stopped at its account's plan limit";

/// The folders of one QA start, under `root`.
pub struct Folders {
  pub root: PathBuf,
}

impl Folders {
  /// The stand-ins connections run.
  pub fn programs(&self) -> PathBuf {
    self.root.join("programs")
  }

  /// The stand-ins on the window's `PATH`.
  pub fn path(&self) -> PathBuf {
    self.root.join("path")
  }

  pub fn home(&self) -> PathBuf {
    self.root.join("home")
  }

  pub fn claude_default(&self) -> PathBuf {
    self.home().join(".claude")
  }

  pub fn codex_default(&self) -> PathBuf {
    self.home().join(".codex")
  }

  /// Where Orchester makes account folders (`ORCHESTER_ACCOUNTS_DIR`).
  pub fn accounts(&self) -> PathBuf {
    self.root.join("accounts")
  }
}

/// The seeded data the window opens.
pub struct Prepared {
  pub database: PathBuf,
  pub project: ProjectId,
  /// The key of the task stopped at its account's plan limit.
  pub task: String,
  /// What the start prints: the scenario and that task's title.
  pub shown: String,
}

/// Writes the stand-ins, seeds the project and adds the accounts and their connections.
pub fn prepare(folders: &Folders, without_codex: bool) -> Result<Prepared, Box<dyn Error>> {
  stand_ins::write(&folders.programs(), false)?;
  stand_ins::write(&folders.path(), without_codex)?;
  for folder in [folders.claude_default(), folders.codex_default(), folders.accounts()] {
    fs::create_dir_all(folder)?;
  }
  let scenario = kontur::scenarios::named(SCENARIO).ok_or("the seed has the accounts scenario")?;
  let seeded = kontur::seed(&kontur::SeedRequest {
    data_dir: folders.root.join("app"),
    repo: folders.root.join("repo-kontur"),
    scenario,
    fake_codex: Some(folders.programs().join(stand_ins::CODEX)),
    now: Timestamp::now(),
  })?;
  let stopped = seeded
    .tasks
    .iter()
    .find(|task| task.state == PLAN_LIMIT_STATE)
    .ok_or("the seed stops a task at its account's plan limit")?;
  let (task, shown) = (
    stopped.key.clone(),
    format!("{} · {} {}", scenario.name(), stopped.key, stopped.title),
  );
  let used = accounts::add(folders, &seeded.app_database)?;
  let mut project = ProjectDb::open(&seeded.repo)?;
  for (name, provider, account, sign_in) in [
    (
      "Claude · Work",
      AgentProvider::ClaudeCodeCli,
      &used.work,
      SignIn::SignedIn,
    ),
    (
      "Codex · Client",
      AgentProvider::CodexCli,
      &used.client,
      SignIn::SignedIn,
    ),
    (
      "Codex · Freelance",
      AgentProvider::CodexCli,
      &used.freelance,
      SignIn::SignedOut,
    ),
  ] {
    let program = folders.programs().join(match provider {
      AgentProvider::ClaudeCodeCli => stand_ins::CLAUDE,
      _ => stand_ins::CODEX,
    });
    project.create_connection(&connection((name, provider), program, account, sign_in)?)?;
  }
  let running = running_run(&project)?;
  on_work(&mut project, running, &used.work)?;
  for (account, provider, used) in [
    (&used.work, AgentProvider::ClaudeCodeCli, (12.0, 41.0)),
    (&used.client, AgentProvider::CodexCli, (34.0, 63.0)),
  ] {
    project.save_limit_snapshot(running, &reading(account, provider, used)?)?;
  }
  Ok(Prepared {
    database: seeded.app_database,
    project: seeded.project,
    task,
    shown,
  })
}

/// The seed's running run.
fn running_run(project: &ProjectDb) -> Result<RunId, Box<dyn Error>> {
  project
    .unfinished_runs()?
    .into_iter()
    .filter_map(Result::ok)
    .find(|run| *run.status() == RunStatus::Running)
    .map(|run| run.id())
    .ok_or_else(|| "the seed has a running run".into())
}

/// Records that the running run's agent started on Work, as the runner records it.
fn on_work(project: &mut ProjectDb, run: RunId, work: &AgentAccount) -> Result<(), Box<dyn Error>> {
  let invocation = project
    .invocations(run)?
    .last()
    .map(|invocation| invocation.id)
    .ok_or("the running run has an agent")?;
  project.save_invocation_account(invocation, work.id())?;
  Ok(())
}

/// A reading of `account`'s limits half an hour ago: its five-hour and weekly windows `used` percent used.
fn reading(account: &AgentAccount, provider: AgentProvider, used: (f64, f64)) -> Result<LimitReading, Box<dyn Error>> {
  let now = Timestamp::now();
  let later = |duration: Duration| {
    Timestamp::from_unix_millis(now.unix_millis() + i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
  };
  let windows = vec![
    LimitWindow::new(
      300,
      Some(used.0),
      Some(later(Duration::from_secs(3 * 60 * 60))),
      LimitState::Normal,
      false,
    )?,
    LimitWindow::new(
      10_080,
      Some(used.1),
      Some(later(Duration::from_secs(3 * 24 * 60 * 60))),
      LimitState::Normal,
      false,
    )?,
  ];
  Ok(LimitReading {
    provider,
    account: Some(account.id()),
    at: Timestamp::from_unix_millis(now.unix_millis() - 30 * 60 * 1000),
    limits: PlanLimits {
      windows,
      plan: None,
      credits: None,
      blocked: None,
    },
  })
}

/// A connection called `name` of `provider` that runs `program` on `account`, checked just now as `sign_in` says.
fn connection(
  (name, provider): (&str, AgentProvider),
  program: PathBuf,
  account: &AgentAccount,
  sign_in: SignIn,
) -> Result<orchester_core::connection::Connection, Box<dyn Error>> {
  let now = Timestamp::now();
  let mut connection = AgentConnection::new(ConnectionId::generate(), Name::new(name)?, provider, program, now)?;
  // Choosing an account drops the connection's check, which is then of the account.
  connection.set_account(ConnectionAccount::of(account))?;
  connection.record_check(ConnectionCheck {
    at: now,
    result: CheckResult::Available {
      version: "stand-in".to_owned(),
      sign_in,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  })?;
  Ok(connection.into_connection())
}
