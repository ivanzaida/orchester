//! The connected accounts the `usage` scenarios draw by (`Usage · many accounts` `u2cb1M`): beside each CLI's
//! Default, Codex's *Client* and Claude Code's *Work* and *Personal* — five, so the fourth and fifth share Other
//! accounts — and which account each invocation started on: Codex's on Client twice for every once on Default,
//! Claude Code's on Work, Work, Personal and Default in turn, and every seventh on none, as before Orchester
//! recorded accounts. The names are illustrative.

use std::path::Path;

use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  id::AgentAccountId,
  name::Name,
  run::{InvocationKind, Run},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::AppDb;

use crate::kontur::{Kontur, SeedError};

/// Every seventh invocation started on no account.
const UNRECORDED_EVERY: usize = 7;

/// The accounts invocations start on, by CLI, in turn.
pub struct SeedAccounts {
  codex: [AgentAccountId; 3],
  claude: [AgentAccountId; 4],
  /// Claude Code's *Work*, whose plan limits the scenario reports.
  pub work: AgentAccountId,
  /// Invocations given an account or none so far: all of them, Codex's and Claude Code's.
  turns: (usize, usize, usize),
}

impl SeedAccounts {
  /// Adds the named accounts to the application database, each with a folder of its own beside it.
  pub fn add(kontur: &Kontur) -> Result<Self, SeedError> {
    let folders = kontur
      .app_database
      .parent()
      .map_or_else(|| Path::new("accounts").to_path_buf(), |folder| folder.join("accounts"));
    let mut db = AppDb::open(&kontur.app_database)?;
    let mut add = |cli: AccountCli, name: &str, minutes: i64| -> Result<AgentAccountId, SeedError> {
      let account = AgentAccount::new(
        AgentAccountId::generate(),
        cli,
        Name::new(name).expect("the seed's names are valid"),
        AccountFolder::Created(folders.join(name.to_lowercase())),
        Timestamp::from_unix_millis(kontur.now.unix_millis() - minutes * 60_000),
      )
      .map_err(|error| SeedError::Invalid(error.to_string()))?;
      db.add_agent_account(&account)?;
      Ok(account.id())
    };
    let client = add(AccountCli::Codex, "Client", 3_000)?;
    let work = add(AccountCli::ClaudeCode, "Work", 2_000)?;
    let personal = add(AccountCli::ClaudeCode, "Personal", 1_000)?;
    let (codex_default, claude_default) = (
      AccountCli::Codex.default_account(),
      AccountCli::ClaudeCode.default_account(),
    );
    Ok(Self {
      codex: [client, client, codex_default],
      claude: [work, work, personal, claude_default],
      work,
      turns: (0, 0, 0),
    })
  }

  /// Records which account each of `run`'s agent invocations started on, in turn.
  pub fn record(&mut self, kontur: &mut Kontur, run: &Run) -> Result<(), SeedError> {
    for invocation in kontur.db.invocations(run.id())? {
      let InvocationKind::Agent { model } = &invocation.kind else {
        continue;
      };
      let (all, codex, claude) = &mut self.turns;
      *all += 1;
      if all.is_multiple_of(UNRECORDED_EVERY) {
        continue;
      }
      let account = if model.as_str().starts_with("claude") {
        *claude += 1;
        self.claude[*claude % self.claude.len()]
      } else {
        *codex += 1;
        self.codex[*codex % self.codex.len()]
      };
      kontur.db.save_invocation_account(invocation.id, account)?;
    }
    Ok(())
  }
}
