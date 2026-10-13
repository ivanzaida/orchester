//! The QA window's accounts, one per state Settings › Accounts names (`E96CI`, `GE7a3`, `yEJPZ`), each with the check
//! a window before this one stored, as an install has: the screens that list accounts show who is signed in before
//! Settings › Accounts asks the CLIs again.

use std::{error::Error, fs, path::Path};

use orchester_core::{
  account::{
    AccountCheck, AccountCheckResult, AccountCli, AccountFolder, AccountIdentity, AgentAccount, CheckedProgram,
    SignInMethod,
  },
  id::AgentAccountId,
  name::Name,
  time::Timestamp,
};
use orchester_store::AppDb;

use super::{data::Folders, stand_ins};

/// Where an account's folder is.
#[derive(Clone, Copy)]
enum Place {
  /// A folder Orchester made, under the QA folder's accounts.
  Created,
  /// A folder Orchester made that is gone.
  Gone,
  /// A dot folder of the QA home the person brought.
  Brought(&'static str),
}

/// What the stand-in CLI says of the folder now.
#[derive(Clone, Copy)]
enum Now {
  SignedIn(&'static str, &'static str),
  SignedOut,
  Garbled,
}

/// What the stored check said: signed in as someone, signed out (after someone was), an answer it couldn't read, or
/// the folder gone, each with who was known.
#[derive(Clone, Copy)]
enum Before {
  SignedIn(&'static str, &'static str),
  SignedOut,
  Expired(&'static str, &'static str),
  Unknown(&'static str, &'static str),
  FolderGone(&'static str, &'static str),
}

const ACCOUNTS: [(AccountCli, &str, Place, Now, Before); 11] = [
  (
    AccountCli::ClaudeCode,
    "Work",
    Place::Created,
    Now::SignedIn("ana@example.com", "max"),
    Before::SignedIn("ana@example.com", "max"),
  ),
  (
    AccountCli::ClaudeCode,
    "Personal",
    Place::Created,
    Now::SignedIn("ana.home@example.com", "pro"),
    Before::SignedIn("ana.home@example.com", "pro"),
  ),
  // The same person as Personal in a second folder: allowed, said plainly.
  (
    AccountCli::ClaudeCode,
    "Spare",
    Place::Created,
    Now::SignedIn("ana.home@example.com", "pro"),
    Before::SignedIn("ana.home@example.com", "pro"),
  ),
  (
    AccountCli::ClaudeCode,
    "Old",
    Place::Created,
    Now::SignedOut,
    Before::Expired("ana@oldcorp.example", "pro"),
  ),
  (
    AccountCli::ClaudeCode,
    "Lab",
    Place::Created,
    Now::Garbled,
    Before::Unknown("ana.lab@example.com", "pro"),
  ),
  (
    AccountCli::ClaudeCode,
    "Mine",
    Place::Brought(".claude-mine"),
    Now::SignedIn("ana.mine@example.com", "pro"),
    Before::SignedIn("ana.mine@example.com", "pro"),
  ),
  (
    AccountCli::Codex,
    "Client",
    Place::Created,
    Now::SignedIn("", ""),
    Before::SignedIn("", ""),
  ),
  (
    AccountCli::Codex,
    "Freelance",
    Place::Created,
    Now::SignedOut,
    Before::SignedOut,
  ),
  (
    AccountCli::Codex,
    "Archive",
    Place::Gone,
    Now::SignedOut,
    Before::FolderGone("ana.archive@example.com", "plus"),
  ),
  // Codex's Default: the CLI's own folder, signed in.
  (
    AccountCli::Codex,
    "",
    Place::Created,
    Now::SignedIn("", ""),
    Before::SignedIn("", ""),
  ),
  // Claude Code's Default: the CLI's own folder, which nobody signed in to.
  (
    AccountCli::ClaudeCode,
    "",
    Place::Created,
    Now::SignedOut,
    Before::SignedOut,
  ),
];

/// The accounts the project's connections run on.
pub struct Used {
  pub work: AgentAccount,
  pub client: AgentAccount,
  pub freelance: AgentAccount,
}

/// Adds the accounts to the application database at `database`, with their folders and their stored checks.
pub fn add(folders: &Folders, database: &Path) -> Result<Used, Box<dyn Error>> {
  let mut db = AppDb::open(database)?;
  let mut added = Vec::new();
  for (cli, name, place, now, before) in ACCOUNTS {
    let mut account = if name.is_empty() {
      AgentAccount::default_of(cli)
    } else {
      let account = new_account(folders, (cli, name), place)?;
      db.add_agent_account(&account)?;
      account
    };
    let folder = match account.folder().path() {
      Some(folder) => folder.to_path_buf(),
      None => folders.home().join(cli.own_folder_name()),
    };
    mark(&folder, place, now)?;
    account.record_check(check(folders, cli, before));
    db.save_agent_account(&account)?;
    added.push(account);
  }
  let named = |name: &str| {
    added
      .iter()
      .find(|account| account.name().is_some_and(|known| known.as_str() == name))
      .cloned()
      .ok_or_else(|| format!("no account {name}"))
  };
  Ok(Used {
    work: named("Work")?,
    client: named("Client")?,
    freelance: named("Freelance")?,
  })
}

fn new_account(
  folders: &Folders,
  (cli, name): (AccountCli, &str),
  place: Place,
) -> Result<AgentAccount, Box<dyn Error>> {
  let folder = match place {
    Place::Created | Place::Gone => AccountFolder::Created(folders.accounts().join(format!(
      "{}-{}",
      cli.program(),
      name.to_lowercase()
    ))),
    Place::Brought(dot) => AccountFolder::Brought(folders.home().join(dot)),
  };
  Ok(AgentAccount::new(
    AgentAccountId::generate(),
    cli,
    Name::new(name)?,
    folder,
    Timestamp::now(),
  )?)
}

/// Makes the folder (unless it is gone) and marks it for the stand-ins as `now` says.
fn mark(folder: &Path, place: Place, now: Now) -> Result<(), Box<dyn Error>> {
  if matches!(place, Place::Gone) {
    return Ok(());
  }
  fs::create_dir_all(folder)?;
  match now {
    Now::SignedIn(email, plan) => stand_ins::sign_in(folder, (email, plan))?,
    Now::Garbled => stand_ins::garble(folder)?,
    Now::SignedOut => {}
  }
  Ok(())
}

/// The check a window stored a day ago, as `before` says.
fn check(folders: &Folders, cli: AccountCli, before: Before) -> AccountCheck {
  let (program, version, method) = match cli {
    AccountCli::ClaudeCode => (stand_ins::CLAUDE, "2.1.296", SignInMethod::ClaudeAi),
    AccountCli::Codex => (stand_ins::CODEX, "0.159.2", SignInMethod::ChatGpt),
  };
  let program = CheckedProgram {
    path: folders.path().join(program),
    version: version.to_owned(),
  };
  let some = |text: &str| (!text.is_empty()).then(|| text.to_owned());
  let identity = |(email, plan): (&str, &str)| AccountIdentity {
    email: some(email),
    organization: None,
    plan: some(plan),
    method: Some(method),
  };
  let result = match before {
    Before::SignedIn(email, plan) => AccountCheckResult::SignedIn {
      program,
      identity: identity((email, plan)),
    },
    Before::SignedOut => AccountCheckResult::SignedOut { program, was: None },
    Before::Expired(email, plan) => AccountCheckResult::SignedOut {
      program,
      was: Some(identity((email, plan))),
    },
    Before::Unknown(email, plan) => AccountCheckResult::SignInUnknown {
      program,
      last_known: Some(identity((email, plan))),
    },
    Before::FolderGone(email, plan) => AccountCheckResult::FolderMissing {
      last_known: Some(identity((email, plan))),
    },
  };
  AccountCheck {
    at: Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 24 * 60 * 60 * 1000),
    result,
  }
}
