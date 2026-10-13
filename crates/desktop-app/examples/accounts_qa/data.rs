//! What the QA window shows: the seed's `task-states` project (its Codex connection on Default, a run stopped at
//! Default's plan limit) and accounts of both CLIs in each state Settings › Accounts names, with two more connections
//! on them. Every folder is under the QA folder: the CLIs' own folders too.

use std::{
  error::Error,
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  account::{
    AccountCheck, AccountCheckResult, AccountCli, AccountFolder, AccountIdentity, AgentAccount, CheckedProgram,
  },
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionAccount, ConnectionCheck, ModelCatalog,
    SignIn,
  },
  id::{AgentAccountId, ConnectionId, ProjectId},
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};

use super::{kontur, stand_ins};

/// The task the seed stopped at its account's plan limit.
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
}

/// Writes the stand-ins, seeds the project and adds the accounts and their connections.
pub fn prepare(folders: &Folders, without_codex: bool) -> Result<Prepared, Box<dyn Error>> {
  stand_ins::write(&folders.programs(), false)?;
  stand_ins::write(&folders.path(), without_codex)?;
  for folder in [folders.claude_default(), folders.codex_default(), folders.accounts()] {
    fs::create_dir_all(folder)?;
  }
  stand_ins::sign_in(&folders.codex_default())?;
  let seeded = kontur::seed(&kontur::SeedRequest {
    data_dir: folders.root.join("app"),
    repo: folders.root.join("repo-kontur"),
    scenario: kontur::scenarios::named("task-states").ok_or("the seed has task-states")?,
    fake_codex: Some(folders.programs().join(stand_ins::CODEX)),
    now: Timestamp::now(),
  })?;
  let task = seeded
    .tasks
    .iter()
    .find(|task| task.state == PLAN_LIMIT_STATE)
    .ok_or("the seed stops a task at its account's plan limit")?
    .key
    .clone();
  let accounts = add_accounts(folders, &seeded.app_database)?;
  let mut project = ProjectDb::open(&seeded.repo)?;
  for (name, provider, account) in [
    ("Claude · Work", AgentProvider::ClaudeCodeCli, &accounts.work),
    ("Codex · Client", AgentProvider::CodexCli, &accounts.client),
  ] {
    let program = folders.programs().join(match provider {
      AgentProvider::ClaudeCodeCli => stand_ins::CLAUDE,
      _ => stand_ins::CODEX,
    });
    project.create_connection(&connection(name, provider, program, account)?)?;
  }
  Ok(Prepared {
    database: seeded.app_database,
    project: seeded.project,
    task,
  })
}

/// The accounts the connections run on.
struct Used {
  work: AgentAccount,
  client: AgentAccount,
}

/// Claude Code: Work and Personal signed in to the same account, Old whose sign-in expired, Lab whose CLI now answers
/// in a way Orchester can't read, and Default not signed in. Codex: Default and Client signed in, Freelance not signed
/// in, and Archive whose folder is gone.
fn add_accounts(folders: &Folders, database: &Path) -> Result<Used, Box<dyn Error>> {
  let mut db = AppDb::open(database)?;
  let mut add = |cli: AccountCli, name: &str, mark: Option<fn(&Path) -> std::io::Result<()>>| {
    add_account(&mut db, folders, (cli, name), mark)
  };
  let work = add(AccountCli::ClaudeCode, "Work", Some(stand_ins::sign_in))?;
  add(AccountCli::ClaudeCode, "Personal", Some(stand_ins::sign_in))?;
  let old = add(AccountCli::ClaudeCode, "Old", Some(|_| Ok(())))?;
  let lab = add(AccountCli::ClaudeCode, "Lab", Some(stand_ins::garble))?;
  let client = add(AccountCli::Codex, "Client", Some(stand_ins::sign_in))?;
  add(AccountCli::Codex, "Freelance", Some(|_| Ok(())))?;
  let archive = add(AccountCli::Codex, "Archive", None)?;
  // Who was signed in when each was last read, which their rows show.
  for (mut account, email) in [
    (old, "ana@oldcorp.example"),
    (lab, "ana.lab@example.com"),
    (archive, "ana.archive@example.com"),
  ] {
    account.record_check(signed_in_before(folders, account.cli(), email));
    db.save_agent_account(&account)?;
  }
  Ok(Used { work, client })
}

/// Adds `cli`'s account `name` in a folder Orchester made; `mark` makes the folder and marks it for the stand-ins,
/// `None` leaves it missing.
fn add_account(
  db: &mut AppDb,
  folders: &Folders,
  (cli, name): (AccountCli, &str),
  mark: Option<fn(&Path) -> std::io::Result<()>>,
) -> Result<AgentAccount, Box<dyn Error>> {
  let folder = folders
    .accounts()
    .join(format!("{}-{}", cli.as_str(), name.to_lowercase()));
  if let Some(mark) = mark {
    fs::create_dir_all(&folder)?;
    mark(&folder)?;
  }
  let account = AgentAccount::new(
    AgentAccountId::generate(),
    cli,
    Name::new(name)?,
    AccountFolder::Created(folder),
    Timestamp::now(),
  )?;
  db.add_agent_account(&account)?;
  Ok(account)
}

/// A check a day ago, when `email` was signed in with `cli`.
fn signed_in_before(folders: &Folders, cli: AccountCli, email: &str) -> AccountCheck {
  let (program, version, method) = match cli {
    AccountCli::ClaudeCode => (stand_ins::CLAUDE, "2.1.296", "claude.ai"),
    AccountCli::Codex => (stand_ins::CODEX, "0.159.2", "ChatGPT"),
  };
  AccountCheck {
    at: Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 24 * 60 * 60 * 1000),
    result: AccountCheckResult::SignedIn {
      program: CheckedProgram {
        path: folders.path().join(program),
        version: version.to_owned(),
      },
      identity: AccountIdentity {
        email: Some(email.to_owned()),
        organization: None,
        plan: Some("pro".to_owned()),
        method: Some(method.to_owned()),
      },
    },
  }
}

/// A connection called `name` that runs `program` on `account`, checked just now.
fn connection(
  name: &str,
  provider: AgentProvider,
  program: PathBuf,
  account: &AgentAccount,
) -> Result<orchester_core::connection::Connection, Box<dyn Error>> {
  let now = Timestamp::now();
  let mut connection = AgentConnection::new(ConnectionId::generate(), Name::new(name)?, provider, program, now)?;
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
