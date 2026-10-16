//! The person's agent CLI accounts, for the screens that name a connection's account (Connections, agent profiles,
//! pipeline steps, a run's summary). Read once when such a screen shows; Settings › Accounts reads its own.

use std::sync::Arc;

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, StreamEmitter},
};
use orchester_core::{
  account::{AccountState, AgentAccount, same_sign_in},
  connection::{AgentProvider, ConnectionAccount, invocation::Platform, plan_limits::LimitReading},
  id::AgentAccountId,
  time::Timestamp,
};
use orchester_workspace::{background, messages, workspace::Workspace};

use super::Services;

/// The accounts as read; empty until they are, or when they can't be.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accounts(pub Arc<Vec<AgentAccount>>);

impl DevtoolsInspectable for Accounts {}

impl Accounts {
  pub fn of(&self, id: AgentAccountId) -> Option<&AgentAccount> {
    self.0.iter().find(|account| account.id() == id)
  }

  /// The account a connection of `provider` runs on; `None` without one, or for a native connection.
  pub fn of_connection(&self, account: &ConnectionAccount, provider: AgentProvider) -> Option<&AgentAccount> {
    self.of(account.id(provider)?)
  }

  /// The accounts of `provider`'s CLI, Default last (`hgOl2`).
  pub fn of_provider(&self, provider: AgentProvider) -> impl Iterator<Item = &AgentAccount> {
    let of = move |account: &&AgentAccount| account.cli().provider() == provider;
    let added = self.0.iter().filter(of).filter(|account| !account.is_default());
    added.chain(self.0.iter().filter(of).filter(|account| account.is_default()))
  }
}

/// A plan as the CLI names it (`max`, `pro`), as people read it: `Max`, `Pro`.
pub fn plan_name(plan: &str) -> String {
  let mut letters = plan.chars();
  letters
    .next()
    .map(|first| first.to_uppercase().chain(letters).collect())
    .unwrap_or_default()
}

/// The accounts, read when the calling screen shows and again whenever one changed ([`changes`]).
pub fn listed(ctx: &mut Ctx) -> Accounts {
  let workspace = Services::of(ctx).workspace;
  let changed = changes(ctx, &workspace);
  ctx
    .future(changed, move |_| {
      tracing::trace!(target: crate::app_log::READ_LOG, read = "accounts");
      let workspace = workspace.clone();
      async move {
        background::run(move || workspace.agent_accounts())
          .await?
          .map(|accounts| Accounts(Arc::new(accounts)))
          .map_err(|error| {
            tracing::warn!(%error, "the accounts can't be read for a connection's account");
            messages::store_error(&error)
          })
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// How often an account was added, renamed, checked, moved or removed, through Settings, a screen or automation.
/// Reading it in a render renders again when one was.
pub fn changes(ctx: &mut Ctx, workspace: &Workspace) -> u64 {
  let changes = ctx.stream(
    workspace.clone(),
    |workspace, emitter: StreamEmitter<u64, ()>| async move {
      let mut updates = workspace.account_changes();
      loop {
        let count = *updates.borrow_and_update();
        if !emitter.emit(count) || updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  // Until the stream's first value: the count now, so a screen that mounts after a change reads once.
  changes
    .state()
    .get()
    .data
    .unwrap_or_else(|| *workspace.account_changes().borrow())
}

/// A week, in minutes: the window an account's use is read from.
const WEEK: u64 = 10_080;

/// The account's name as people read it: the one given, or *Default*.
pub fn account_name(ctx: &Ctx, account: &AgentAccount) -> String {
  account.name().map_or_else(
    || ctx.t("connections.account.default").to_string(),
    |name| name.as_str().to_owned(),
  )
}

/// What keeps runs from starting on `account`, in a few words (`M42rM`: "sign-in expired"); `None` when nothing does.
pub fn short_state(ctx: &Ctx, account: &AgentAccount) -> Option<String> {
  let key = match account.state(Platform::current()) {
    AccountState::SignedIn | AccountState::Unknown | AccountState::NotChecked => return None,
    AccountState::NotSignedIn => "connections.account.short.notSignedIn",
    AccountState::Expired => "connections.account.short.expired",
    AccountState::CliMissing => "connections.account.short.cliMissing",
    AccountState::FolderMissing => "connections.account.short.folderMissing",
    AccountState::Unsupported => "connections.account.short.unsupported",
  };
  Some(ctx.t(key).to_string())
}

/// Where an account's detail stands: on a line of its own, or after a comma in a sentence (`account Work, …`), where
/// it starts in lower case.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DetailPlace {
  Line,
  AfterComma,
}

/// Who is signed in to `account`, its plan and the week's use (`WaKJN`): or the account it shares a sign-in with, or
/// that no check has read it yet. `reading` is the account's last plan reading.
pub fn identity_detail(
  ctx: &Ctx,
  (account, accounts): (&AgentAccount, &Accounts),
  reading: Option<&LimitReading>,
  place: DetailPlace,
) -> String {
  let (not_checked, signed_in) = match place {
    DetailPlace::Line => ("connections.account.notChecked", "connections.account.signedIn"),
    DetailPlace::AfterComma => (
      "connections.account.notCheckedInline",
      "connections.account.signedInInline",
    ),
  };
  if account.state(Platform::current()) == AccountState::NotChecked {
    return ctx.t(not_checked).to_string();
  }
  let identity = account.last_check().and_then(|check| check.known_identity());
  let email = identity.and_then(|identity| identity.email.clone());
  if let (Some(other), Some(email)) = (same_sign_in(account, &accounts.0), email.clone()) {
    return ctx
      .t_args(
        "connections.account.same",
        [("email", email), ("account", account_name(ctx, other))],
      )
      .to_string();
  }
  let mut parts: Vec<String> = email.into_iter().collect();
  if let Some(plan) = identity.and_then(|identity| identity.plan.as_deref()) {
    parts.push(
      ctx
        .t_args("connections.account.plan", [("plan", plan_name(plan))])
        .to_string(),
    );
  }
  if let Some(used) = weekly_use(reading) {
    parts.push(
      ctx
        .t_args("connections.account.weekly", [("used", used.to_string())])
        .to_string(),
    );
  }
  if parts.is_empty() {
    parts.push(ctx.t(signed_in).to_string());
  }
  parts.join(&*ctx.t("connections.account.separator"))
}

/// The share of the account's weekly window used, in whole percent, as its CLI last reported it.
pub fn weekly_use(reading: Option<&LimitReading>) -> Option<u64> {
  let now = Timestamp::now();
  let weekly = reading?
    .limits
    .windows
    .iter()
    .find(|window| window.minutes() == WEEK && !window.expired(now))?;
  // A share of at most a few hundred percent: whole ones are what a menu shows.
  weekly.percent().map(|used| used.round() as u64)
}
