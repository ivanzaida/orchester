//! Plan limits read on their own (`docs/design/agent-accounts.md`, "Plan limits read on their own"): the reader the
//! app starts ([`PlanReads::run`]) reads each Codex account through App Server as [`ReadSchedule`] says, and a Claude
//! Code account only when the person asks (*Check now*, [`PlanReads::check_now`]), since that spends a few tokens. A
//! reading goes to the session's readings beside the turns' ([`Runners::observe_plan_limits`]) and into the
//! application database, where the Usage page's history reads it.
//!
//! Only the app's own window starts the reader: a test or a QA copy has none, so it never reads a real account, and
//! its *Check now* says so.

use std::{
  collections::HashSet,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  account::{AccountCli, AccountState, AgentAccount},
  command::{CommandError, CommandRunner},
  connection::plan_limits::{
    LimitReading, PlanLimits, ReadingSource,
    read::{ReadError, claude_check_command, codex_read_command, read_claude_check, read_codex_limits},
    schedule::{Outcome, ReadSchedule, Trigger},
  },
  id::AgentAccountId,
  time::Timestamp,
};
use orchester_exec::ProcessRunner;
use parking_lot::Mutex;
use tokio::sync::Notify;

use crate::{
  accounts::cli_environment, background, connections::probe, messages::Message, runner::Runners, workspace::Workspace,
};

/// A CLI that does not answer a read within this time counts as failed.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// *Check now* waits for a whole model turn.
const CHECK_TIMEOUT: Duration = Duration::from_secs(120);

/// The reads of the session's accounts. Clones share them.
#[derive(Clone)]
pub struct PlanReads {
  inner: Arc<Inner>,
}

struct Inner {
  workspace: Workspace,
  runners: Runners,
  schedule: Mutex<ReadSchedule>,
  /// How many of the status bar's plan limits hint and the Usage pages are shown.
  shown: AtomicUsize,
  /// Set once [`PlanReads::run`] started: nothing is read before, and nothing at all without it.
  running: AtomicBool,
  wake: Notify,
  /// The Claude Code accounts a *Check now* runs for.
  checking: Mutex<HashSet<AgentAccountId>>,
}

impl PartialEq for PlanReads {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

/// Held while the hint or a Usage page is shown: the accounts are read when it opens and every few minutes while any
/// is held.
pub struct Shown(Arc<Inner>);

impl Drop for Shown {
  fn drop(&mut self) {
    self.0.shown.fetch_sub(1, Ordering::AcqRel);
  }
}

impl PlanReads {
  pub fn new(workspace: Workspace, runners: Runners) -> Self {
    Self {
      inner: Arc::new(Inner {
        workspace,
        runners,
        schedule: Mutex::new(ReadSchedule::default()),
        shown: AtomicUsize::new(0),
        running: AtomicBool::new(false),
        wake: Notify::new(),
        checking: Mutex::new(HashSet::new()),
      }),
    }
  }

  /// The hint or a Usage page opened; it stays shown while the returned guard is held.
  pub fn shown(&self) -> Shown {
    self.inner.shown.fetch_add(1, Ordering::AcqRel);
    self.inner.schedule.lock().request(Trigger::Opened);
    self.inner.wake.notify_one();
    Shown(self.inner.clone())
  }

  /// Whether a *Check now* of `account` is running.
  pub fn checking(&self, account: AgentAccountId) -> bool {
    self.inner.checking.lock().contains(&account)
  }

  /// Reads the accounts as the schedule says until the future is dropped: at once (the app started), when an account
  /// changes, when the hint or the Usage page opens, after a reset the last reading named, and every few minutes while
  /// one is shown.
  pub async fn run(self) {
    self.inner.running.store(true, Ordering::Release);
    let mut changes = self.inner.workspace.account_changes();
    let mut readings = self.inner.runners.plan_limits();
    self.inner.schedule.lock().request(Trigger::Start);
    loop {
      let next = match self.read_due().await {
        Ok(next) => next,
        Err(error) => {
          tracing::warn!(?error, "cannot read the accounts whose plan limits are read");
          None
        }
      };
      let sleep = next.map(|at| {
        // A time already past waits for nothing.
        let left = at.unix_millis().saturating_sub(Timestamp::now().unix_millis());
        Duration::from_millis(u64::try_from(left).unwrap_or(0))
      });
      tokio::select! {
        () = self.inner.wake.notified() => {}
        changed = changes.changed() => {
          if changed.is_err() {
            return;
          }
          self.inner.schedule.lock().request(Trigger::AccountsChanged);
        }
        // A turn's reading may name a new reset to read after.
        changed = readings.changed() => {
          if changed.is_err() {
            return;
          }
        }
        () = sleep_for(sleep) => {}
      }
    }
  }

  /// Reads every account that is due, one after the other; when the next one becomes due.
  async fn read_due(&self) -> Result<Option<Timestamp>, Message> {
    let workspace = self.inner.workspace.clone();
    let accounts = background::run(move || workspace.agent_accounts())
      .await?
      .map_err(|error| crate::messages::store_error(&error))?;
    let readable: Vec<AgentAccount> = accounts.into_iter().filter(read_on_its_own).collect();
    let ids: Vec<AgentAccountId> = readable.iter().map(AgentAccount::id).collect();
    loop {
      let shown = self.inner.shown.load(Ordering::Acquire) > 0;
      let current = self.inner.runners.plan_limits().borrow().clone();
      let (due, next) = {
        let mut schedule = self.inner.schedule.lock();
        schedule.track(&ids);
        let now = Timestamp::now();
        let reading = |id| current.of_account(id);
        (
          schedule.due(now, shown, reading),
          schedule.next_due(now, shown, reading),
        )
      };
      let Some(account) = due
        .first()
        .and_then(|id| readable.iter().find(|account| account.id() == *id))
      else {
        return Ok(next);
      };
      let (id, account) = (account.id(), account.clone());
      let started = Timestamp::now();
      let result = background::run(move || read_codex(&account)).await?;
      let outcome = Outcome::of(result.as_ref().map(|_| ()));
      match result {
        Ok(limits) => self.publish(id, AccountCli::Codex, started, limits).await,
        Err(error) => tracing::info!(%error, account = %id, "the plan limits of an account could not be read"),
      }
      self.inner.schedule.lock().record(id, started, outcome);
    }
  }

  /// Hands a reading of `account` on and keeps it.
  async fn publish(&self, account: AgentAccountId, cli: AccountCli, at: Timestamp, limits: PlanLimits) {
    let workspace = self.inner.workspace.clone();
    let stored = limits.clone();
    match background::run(move || workspace.save_plan_read((account, cli), at, &stored)).await {
      Ok(Ok(())) => {}
      // The session still shows it; only the Usage page's history misses it.
      Ok(Err(error)) => tracing::error!(%error, %account, "cannot keep a plan limits read"),
      Err(error) => tracing::error!(?error, %account, "cannot keep a plan limits read"),
    }
    let reading = LimitReading {
      provider: cli.provider(),
      account: Some(account),
      at,
      limits,
    };
    self.inner.runners.observe_plan_limits(reading, ReadingSource::Read);
  }

  /// *Check now* of a Claude Code account: the smallest turn Claude Code runs, whose plan limits are the reading. It
  /// spends a few tokens of the account's plan, so only the person starts it. Blocks for the turn.
  pub fn check_now(&self, account: AgentAccountId) -> Result<(), Message> {
    if !self.inner.running.load(Ordering::Acquire) {
      return Err(Message::new("accounts.planCheck.off"));
    }
    let account = self
      .inner
      .workspace
      .agent_account(account)
      .map_err(|error| crate::messages::store_error(&error))?
      .ok_or_else(|| Message::new("accounts.planCheck.notClaude"))?;
    if account.cli() != AccountCli::ClaudeCode {
      return Err(Message::new("accounts.planCheck.notClaude"));
    }
    match account.state() {
      AccountState::NotSignedIn | AccountState::Expired => return Err(Message::new("accounts.planCheck.notSignedIn")),
      AccountState::FolderMissing => {
        return Err(Message::new("accounts.planCheck.unavailable"));
      }
      AccountState::NotChecked | AccountState::SignedIn | AccountState::Unknown | AccountState::CliMissing => {}
    }
    if !self.inner.checking.lock().insert(account.id()) {
      return Err(Message::new("accounts.planCheck.busy"));
    }
    let started = Timestamp::now();
    let checked = check_claude(&account);
    self.inner.checking.lock().remove(&account.id());
    let limits = checked.map_err(|error| check_message(&error))?;
    for limits in limits {
      let reading = LimitReading {
        provider: AccountCli::ClaudeCode.provider(),
        account: Some(account.id()),
        at: started,
        limits: limits.clone(),
      };
      self.inner.runners.observe_plan_limits(reading, ReadingSource::Read);
      if let Err(error) = self
        .inner
        .workspace
        .save_plan_read((account.id(), AccountCli::ClaudeCode), started, &limits)
      {
        tracing::error!(%error, account = %account.id(), "cannot keep a plan limits check");
      }
    }
    Ok(())
  }
}

/// Whether `account` is read without being asked: a Codex account someone may be signed in to, whose folder is there.
/// A Claude Code account is read only on *Check now*.
fn read_on_its_own(account: &AgentAccount) -> bool {
  account.cli() == AccountCli::Codex
    && matches!(
      account.state(),
      AccountState::NotChecked | AccountState::SignedIn | AccountState::Unknown
    )
}

/// The program `account` runs: the one its last check ran, else its CLI on the `PATH`.
fn program(account: &AgentAccount) -> Option<PathBuf> {
  account
    .last_check()
    .and_then(|check| check.program())
    .map(|program| program.path.clone())
    .filter(|path| path.is_file())
    .or_else(|| probe::detect(account.cli().program()).into_iter().next())
}

/// Reads the Codex `account`'s limits through App Server.
fn read_codex(account: &AgentAccount) -> Result<PlanLimits, ReadError> {
  let program = program(account).ok_or(ReadError::Command(CommandError::NotFound))?;
  let environment = cli_environment(account).map_err(|error| ReadError::Refused(error.to_string()))?;
  read_codex_limits(
    &ProcessRunner::new(READ_TIMEOUT),
    &codex_read_command(&program, environment),
  )
}

/// Runs *Check now* for the Claude Code `account`, in an empty folder of its own that goes afterwards.
fn check_claude(account: &AgentAccount) -> Result<Vec<PlanLimits>, ReadError> {
  let program = program(account).ok_or(ReadError::Command(CommandError::NotFound))?;
  let environment = cli_environment(account).map_err(|error| ReadError::Refused(error.to_string()))?;
  let folder = std::env::temp_dir().join(format!("orchester-plan-check-{}", AgentAccountId::generate()));
  std::fs::create_dir_all(&folder).map_err(|error| ReadError::Command(CommandError::NotStarted(error.to_string())))?;
  let output = ProcessRunner::new(CHECK_TIMEOUT).run(&claude_check_command(&program, environment, &folder));
  if let Err(error) = std::fs::remove_dir_all(&folder) {
    tracing::warn!(%error, folder = %folder.display(), "an empty plan check folder stays");
  }
  read_claude_check(&output?)
}

/// What the person reads when *Check now* gave no reading.
fn check_message(error: &ReadError) -> Message {
  tracing::warn!(%error, "a plan limits check gave no reading");
  match error {
    ReadError::NotSignedIn => Message::new("accounts.planCheck.notSignedIn"),
    ReadError::NoLimits => Message::new("accounts.planCheck.noLimits"),
    ReadError::NoAnswer => Message::new("accounts.planCheck.noAnswer"),
    ReadError::Command(CommandError::NotFound) => Message::new("accounts.planCheck.cliMissing"),
    ReadError::Command(CommandError::TimedOut) => Message::new("accounts.planCheck.timedOut"),
    ReadError::Command(_) => Message::new("accounts.planCheck.failed"),
    ReadError::RateLimited(reason) | ReadError::Refused(reason) => {
      Message::new("accounts.planCheck.refused").with("reason", reason.clone())
    }
  }
}

/// Sleeps `duration`, or forever without one.
async fn sleep_for(duration: Option<Duration>) {
  match duration {
    Some(duration) => tokio::time::sleep(duration).await,
    None => std::future::pending().await,
  }
}
