//! The sign-ins the runs-on picker lists (`05 Account and model picker` `v6mWp8`): Claude Code's *Work*, signed in on
//! Max with 42 % of its 5-hour window used; Claude Code's *Personal*, signed in on Pro with its 5-hour limit reached
//! for another 80 minutes; and Codex's *Client*, signed out. Each is an account of the application database with the
//! check and the plan reading the picker reads, and a connection of the project runs on it: *Claude · Work* (the
//! Orchestrator's, made by the scenario), *Claude · Personal* and *Codex · Client*. The accounts' folders are names
//! only: nothing runs on them.

use std::path::{Path, PathBuf};

use orchester_core::{
  account::{
    AccountCheck, AccountCheckResult, AccountCli, AccountFolder, AccountIdentity, AgentAccount, CheckedProgram,
  },
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionAccount, ConnectionCheck,
    ModelCatalog, SignIn,
    plan_limits::{LimitState, LimitWindow, PlanLimits},
  },
  id::{AgentAccountId, ConnectionId},
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::AppDb;

use crate::kontur::{Kontur, SeedError, name};

/// The 5-hour window, in minutes.
const FIVE_HOURS: u64 = 300;

/// The accounts' connections: what each runs on.
pub struct SignIns {
  pub work: ConnectionAccount,
}

/// One account to add: its CLI and name, who the check found signed in (`None`: nobody) and its 5-hour use with
/// whether its limit is reached.
struct Planned<'a> {
  cli: AccountCli,
  named: &'a str,
  identity: Option<(&'a str, &'a str)>,
  used: Option<(f64, bool)>,
}

/// Adds the accounts, their checks and readings, and the connections on *Personal* and *Client*; what *Work*'s
/// connection runs on.
pub fn add(kontur: &mut Kontur, programs: (&Path, &Path)) -> Result<SignIns, SeedError> {
  let folders = kontur
    .app_database
    .parent()
    .map_or_else(|| PathBuf::from("accounts"), |folder| folder.join("accounts"));
  let mut db = AppDb::open(&kontur.app_database)?;
  let at = kontur.ago(5);
  let mut account =
    |planned: Planned<'_>| -> Result<AgentAccount, SeedError> { add_account(&mut db, &folders, &planned, at) };
  let work = account(Planned {
    cli: AccountCli::ClaudeCode,
    named: "Work",
    identity: Some(("dev@kontur.example", "max")),
    used: Some((42.0, false)),
  })?;
  let personal = account(Planned {
    cli: AccountCli::ClaudeCode,
    named: "Personal",
    identity: Some(("me@personal.example", "pro")),
    used: Some((100.0, true)),
  })?;
  let client = account(Planned {
    cli: AccountCli::Codex,
    named: "Client",
    identity: None,
    used: None,
  })?;
  connection(
    kontur,
    ("Claude · Personal", AgentProvider::ClaudeCodeCli, programs.0),
    (&personal, &["claude-opus-5-5", "claude-sonnet-5-5"]),
  )?;
  connection(
    kontur,
    ("Codex · Client", AgentProvider::CodexCli, programs.1),
    (&client, &["gpt-5.5"]),
  )?;
  Ok(SignIns {
    work: ConnectionAccount::of(&work),
  })
}

fn add_account(
  db: &mut AppDb,
  folders: &Path,
  planned: &Planned<'_>,
  at: Timestamp,
) -> Result<AgentAccount, SeedError> {
  let mut account = AgentAccount::new(
    AgentAccountId::generate(),
    planned.cli,
    name(planned.named),
    AccountFolder::Created(folders.join(planned.named.to_lowercase())),
    at,
  )
  .map_err(|error| SeedError::Invalid(error.to_string()))?;
  db.add_agent_account(&account)?;
  let program = CheckedProgram {
    path: PathBuf::from(planned.cli.as_str()),
    version: "2.1.89".to_owned(),
  };
  let result = match planned.identity {
    Some((email, plan)) => AccountCheckResult::SignedIn {
      program,
      identity: AccountIdentity {
        email: Some(email.to_owned()),
        plan: Some(plan.to_owned()),
        ..AccountIdentity::default()
      },
    },
    None => AccountCheckResult::SignedOut { program, was: None },
  };
  account.record_check(AccountCheck { at, result });
  db.save_agent_account(&account)?;
  if let Some((percent, reached)) = planned.used {
    let resets = Timestamp::from_unix_millis(at.unix_millis() + 85 * 60_000);
    let state = if reached {
      LimitState::Reached
    } else {
      LimitState::Normal
    };
    let window = LimitWindow::new(FIVE_HOURS, Some(percent), Some(resets), state, false)
      .map_err(|error| SeedError::Invalid(error.to_string()))?;
    let limits = PlanLimits {
      windows: vec![window],
      plan: planned.identity.map(|(_, plan)| plan.to_owned()),
      credits: None,
      blocked: None,
    };
    db.save_plan_read((account.id(), planned.cli), at, &limits)?;
  }
  Ok(account)
}

/// A CLI connection on `account`, checked and signed in, offering `models`.
fn connection(
  kontur: &mut Kontur,
  (named, provider, program): (&str, AgentProvider, &Path),
  (account, models): (&AgentAccount, &[&str]),
) -> Result<ConnectionId, SeedError> {
  let at = kontur.ago(4);
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name(named),
    provider,
    program.to_path_buf(),
    at,
  )?;
  connection.set_account(ConnectionAccount::of(account))?;
  let models = models
    .iter()
    .map(|model| {
      ModelId::new(model)
        .map(|id| CatalogModel::new(id, Vec::new()))
        .map_err(|error| SeedError::Invalid(error.to_string()))
    })
    .collect::<Result<Vec<_>, _>>()?;
  connection.record_check(ConnectionCheck {
    at,
    result: CheckResult::Available {
      version: "2.1.89".to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, models),
    },
  })?;
  let connection = connection.into_connection();
  kontur.db.create_connection(&connection)?;
  Ok(connection.id())
}
