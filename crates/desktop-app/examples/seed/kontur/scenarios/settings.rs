//! `settings`: what app Settings shows with a person's data in it, for design verification (`App settings ·
//! redesign` `iUmo3`), deterministic and fixture-only. The `accounts` project (a running run, so an account it runs on
//! can't be removed); agent accounts of both CLIs in different states, each with the check a window before stored and
//! stand-in CLIs that answer the same when Settings › Accounts checks again (`settings-qa/` beside the database, which
//! `tools/qa-app.ps1` puts first on the QA app's `PATH`, with the CLIs' own folders and home there); plan readings a
//! run reported for Work and Client; connections on Work and Client, so removing Work is blocked by the run, removing
//! Client moves its connection, and Personal is unused; quiet hours on from 22:30 to 07:00 and the daily report at
//! 18:30. Team servers, invites and the installed app's MCP rows are the QA app's own (`examples/qa_app/settings`):
//! they need a cloud and servers that answer while the window is open.

#[allow(
  dead_code,
  clippy::duplicate_mod,
  reason = "the accounts QA's stand-in CLIs, which it includes itself too; the seed marks folders signed in only"
)]
#[path = "../../../accounts_qa/stand_ins.rs"]
mod stand_ins;

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  account::{
    AccountCheck, AccountCheckResult, AccountCli, AccountFolder, AccountIdentity, AgentAccount, CheckedProgram,
    SignInMethod,
  },
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, Connection, ConnectionAccount, ConnectionCheck,
    ModelCatalog, SignIn,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits},
  },
  id::{AgentAccountId, ConnectionId, RunId},
  notification::{NotificationSettings, QuietHours},
  orchestrator::report::ReportTime,
  run::RunStatus,
  store::{ActivityStore, ConnectionStore},
  time::Timestamp,
};
use orchester_store::AppDb;

use crate::kontur::{Kontur, SeedError, io_error as io, name};

/// The folder beside the application database that holds the stand-in CLIs, the home with the CLIs' own folders, and
/// the accounts' folders.
pub const FOLDER: &str = "settings-qa";

/// What the stand-in CLI says of an account's folder now, and what the stored check said.
#[derive(Clone, Copy)]
enum Shown {
  SignedIn(&'static str, &'static str),
  /// Signed in before as the person on the plan; the CLI now says it is not: *Sign-in expired*.
  Expired(&'static str, &'static str),
  /// Nobody signed in: *Not signed in*.
  SignedOut,
  /// The folder is gone: *Folder missing*.
  FolderGone(&'static str, &'static str),
}

/// The named accounts, then each CLI's Default (no name).
const ACCOUNTS: [(AccountCli, &str, Shown); 7] = [
  (
    AccountCli::ClaudeCode,
    "Work",
    Shown::SignedIn("ana@example.com", "max"),
  ),
  (
    AccountCli::ClaudeCode,
    "Personal",
    Shown::SignedIn("ana.home@example.com", "pro"),
  ),
  (
    AccountCli::ClaudeCode,
    "Old",
    Shown::Expired("ana@oldcorp.example", "pro"),
  ),
  (AccountCli::Codex, "Client", Shown::SignedIn("", "")),
  (
    AccountCli::Codex,
    "Archive",
    Shown::FolderGone("ana.archive@example.com", "plus"),
  ),
  (AccountCli::ClaudeCode, "", Shown::SignedOut),
  (AccountCli::Codex, "", Shown::SignedIn("", "")),
];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::accounts::seed(kontur)?;
  let root = folder(&kontur.app_database);
  for folder in [
    root.join("path"),
    home(&root).join(".claude"),
    home(&root).join(".codex"),
    accounts(&root),
  ] {
    fs::create_dir_all(&folder).map_err(io(&folder))?;
  }
  stand_ins::write(&root.join("path"), false).map_err(io(&root))?;
  let mut db = AppDb::open(&kontur.app_database)?;
  let mut added = Vec::new();
  for (cli, named, shown) in ACCOUNTS {
    added.push(add(&mut db, &root, (cli, named, shown), kontur.ago(3 * 24 * 60))?);
  }
  let named = |wanted: &str| {
    added
      .iter()
      .find(|account| account.name().is_some_and(|name| name.as_str() == wanted))
      .cloned()
      .ok_or_else(|| SeedError::Invalid(format!("no account {wanted}")))
  };
  let (work, client) = (named("Work")?, named("Client")?);
  let programs = root.join("path");
  for (title, provider, account) in [
    ("Claude · Work", AgentProvider::ClaudeCodeCli, &work),
    ("Codex · Client", AgentProvider::CodexCli, &client),
  ] {
    let program = programs.join(match provider {
      AgentProvider::ClaudeCodeCli => stand_ins::CLAUDE,
      _ => stand_ins::CODEX,
    });
    kontur
      .db
      .create_connection(&connection((title, provider), program, account, kontur.now)?)?;
  }
  let running = running_run(kontur)?;
  let invocation = kontur
    .db
    .invocations(running)?
    .last()
    .map(|invocation| invocation.id)
    .ok_or_else(|| SeedError::Invalid("the running run has an agent".to_owned()))?;
  kontur.db.save_invocation_account(invocation, work.id())?;
  for (account, provider, used) in [
    (&work, AgentProvider::ClaudeCodeCli, (12.0, 41.0)),
    (&client, AgentProvider::CodexCli, (34.0, 63.0)),
  ] {
    kontur
      .db
      .save_limit_snapshot(running, &reading((account.id(), provider), used, kontur.now)?)?;
  }
  notifications(&mut db, kontur.now)
}

/// The stand-ins' folder beside the application database at `database`.
pub fn folder(database: &Path) -> PathBuf {
  database.parent().unwrap_or(Path::new(".")).join(FOLDER)
}

/// The home the QA app runs with, which holds the CLIs' own folders.
pub fn home(root: &Path) -> PathBuf {
  root.join("home")
}

/// Where the accounts' folders are made, and where Add account makes new ones (`ORCHESTER_ACCOUNTS_DIR`).
pub fn accounts(root: &Path) -> PathBuf {
  root.join("accounts")
}

/// Adds one account (a Default only takes its check), marks its folder for the stand-ins and stores the check a
/// window `at` made.
fn add(
  db: &mut AppDb,
  root: &Path,
  (cli, named, shown): (AccountCli, &str, Shown),
  at: Timestamp,
) -> Result<AgentAccount, SeedError> {
  let mut account = if named.is_empty() {
    AgentAccount::default_of(cli)
  } else {
    let folder = accounts(root).join(format!("{}-{}", cli.program(), named.to_lowercase()));
    let account = AgentAccount::new(
      AgentAccountId::generate(),
      cli,
      name(named),
      AccountFolder::Created(folder),
      at,
    )
    .map_err(|error| SeedError::Invalid(error.to_string()))?;
    db.add_agent_account(&account)?;
    account
  };
  let folder = account
    .folder()
    .path()
    .map_or_else(|| home(root).join(cli.own_folder_name()), Path::to_path_buf);
  if !matches!(shown, Shown::FolderGone(..)) {
    fs::create_dir_all(&folder).map_err(io(&folder))?;
  }
  if let Shown::SignedIn(email, plan) = shown {
    stand_ins::sign_in(&folder, (email, plan)).map_err(io(&folder))?;
  }
  account.record_check(check(root, cli, shown, at));
  db.save_agent_account(&account)?;
  Ok(account)
}

/// The check a window stored, as `shown` says.
fn check(root: &Path, cli: AccountCli, shown: Shown, at: Timestamp) -> AccountCheck {
  let (program, version, method) = match cli {
    AccountCli::ClaudeCode => (stand_ins::CLAUDE, "2.1.296", SignInMethod::ClaudeAi),
    AccountCli::Codex => (stand_ins::CODEX, "0.159.2", SignInMethod::ChatGpt),
  };
  let program = CheckedProgram {
    path: root.join("path").join(program),
    version: version.to_owned(),
  };
  let some = |text: &str| (!text.is_empty()).then(|| text.to_owned());
  let identity = |(email, plan): (&str, &str)| AccountIdentity {
    email: some(email),
    organization: None,
    plan: some(plan),
    method: Some(method),
  };
  let result = match shown {
    Shown::SignedIn(email, plan) => AccountCheckResult::SignedIn {
      program,
      identity: identity((email, plan)),
    },
    Shown::Expired(email, plan) => AccountCheckResult::SignedOut {
      program,
      was: Some(identity((email, plan))),
    },
    Shown::SignedOut => AccountCheckResult::SignedOut { program, was: None },
    Shown::FolderGone(email, plan) => AccountCheckResult::FolderMissing {
      last_known: Some(identity((email, plan))),
    },
  };
  AccountCheck { at, result }
}

/// The seed's running run.
fn running_run(kontur: &Kontur) -> Result<RunId, SeedError> {
  kontur
    .db
    .unfinished_runs()?
    .into_iter()
    .filter_map(Result::ok)
    .find(|run| *run.status() == RunStatus::Running)
    .map(|run| run.id())
    .ok_or_else(|| SeedError::Invalid("the accounts scenario has a running run".to_owned()))
}

/// A reading of `account`'s limits half an hour before `now`: its five-hour and weekly windows `used` percent used.
fn reading(
  (account, provider): (AgentAccountId, AgentProvider),
  used: (f64, f64),
  now: Timestamp,
) -> Result<LimitReading, SeedError> {
  let later = |minutes: i64| Timestamp::from_unix_millis(now.unix_millis() + minutes * 60_000);
  let window = |minutes: u64, used: f64, resets: i64| {
    LimitWindow::new(minutes, Some(used), Some(later(resets)), LimitState::Normal, false)
      .map_err(|error| SeedError::Invalid(error.to_string()))
  };
  Ok(LimitReading {
    provider,
    account: Some(account),
    at: Timestamp::from_unix_millis(now.unix_millis() - 30 * 60_000),
    limits: PlanLimits {
      windows: vec![window(300, used.0, 3 * 60)?, window(10_080, used.1, 3 * 24 * 60)?],
      plan: None,
      credits: None,
      blocked: None,
    },
  })
}

/// A connection called `title` of `provider` that runs `program` on `account`, checked at `now`.
fn connection(
  (title, provider): (&str, AgentProvider),
  program: PathBuf,
  account: &AgentAccount,
  now: Timestamp,
) -> Result<Connection, SeedError> {
  let mut connection = AgentConnection::new(ConnectionId::generate(), name(title), provider, program, now)?;
  // Choosing an account drops the connection's check, which is then of the account.
  connection.set_account(ConnectionAccount::of(account))?;
  connection.record_check(ConnectionCheck {
    at: now,
    result: CheckResult::Available {
      version: "stand-in".to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  })?;
  Ok(connection.into_connection())
}

/// Quiet hours on from 22:30 to 07:00, the other notifications as a new install has them, and the daily report at
/// 18:30.
fn notifications(db: &mut AppDb, now: Timestamp) -> Result<(), SeedError> {
  let quiet = QuietHours::new(22 * 60 + 30, 7 * 60).map_err(|error| SeedError::Invalid(error.to_string()))?;
  db.set_notification_settings(NotificationSettings {
    quiet: Some(quiet),
    ..NotificationSettings::default()
  })?;
  let time = ReportTime::new(18 * 60 + 30).ok_or_else(|| SeedError::Invalid("18:30 is a time of day".to_owned()))?;
  db.set_daily_report(Some(time), now)?;
  Ok(())
}
