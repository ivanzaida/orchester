//! Account-specific task decisions and their seeded connections.

use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionAccount, ConnectionCheck, ModelCatalog,
    SignIn,
  },
  id::{AgentAccountId, ConnectionId},
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{OFFLINE, Page};

impl Page {
  /// `task-states` with its Codex connection (on Default, its program a file that is never run) and, with
  /// `other_account`, a second Codex connection, *Codex · Work*, on account Work, whose folder exists.
  fn with_codex(other_account: bool) -> Self {
    Self::with_codex_in("task-states", other_account)
  }

  /// [`Self::with_codex`] on `scenario`.
  fn with_codex_in(scenario: &str, other_account: bool) -> Self {
    Self::seeded_with(scenario, |dir, repo| {
      let Some(repo) = repo else {
        let program = dir.join("codex.exe");
        std::fs::write(&program, "").expect("the stand-in program");
        return Some(program);
      };
      if !other_account {
        return None;
      }
      let folder = dir.join("accounts").join("work");
      std::fs::create_dir_all(&folder).expect("the account's folder");
      let now = Timestamp::now();
      let account = AgentAccount::new(
        AgentAccountId::generate(),
        AccountCli::Codex,
        Name::new("Work").expect("a name"),
        AccountFolder::Created(folder),
        now,
      )
      .expect("an account");
      let mut connection = AgentConnection::new(
        ConnectionId::generate(),
        Name::new("Codex · Work").expect("a name"),
        AgentProvider::CodexCli,
        dir.join("codex.exe"),
        now,
      )
      .expect("a connection");
      // Choosing an account drops the connection's check, which is then of the account.
      connection
        .set_account(ConnectionAccount::of(&account))
        .expect("a Codex account");
      connection
        .record_check(ConnectionCheck {
          at: now,
          result: CheckResult::Available {
            version: "0.139.0".to_owned(),
            sign_in: SignIn::SignedIn,
            catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
          },
        })
        .expect("a check");
      ProjectDb::open(repo)
        .expect("the project database")
        .create_connection(&connection.into_connection())
        .expect("the connection is saved");
      None
    })
  }
}

#[test]
fn decision_plan_limit_offers_the_reset_and_no_other_account_without_one() {
  let mut page = Page::with_codex(false);
  page.open("stopped at its account's plan limit", "plan-limit-choice");
  let notice = page.text("task-notice");
  assert!(notice.starts_with("Stopped at Default's plan limit"), "{notice}");
  assert_eq!(page.text("plan-limit-title"), "Continue this run");
  assert!(page.text("plan-limit-account").starts_with("Default"));
  let windows = page.text("plan-limit-windows");
  assert!(
    windows.contains("5-hour") && windows.contains("limit reached"),
    "{windows}"
  );
  let wait = page.text("plan-limit-wait-title");
  assert_eq!(wait, "Wait for Default, then continue");
  // The seed has one Codex connection, on Default: no other account to continue on.
  assert_eq!(
    page.text("plan-limit-other-reason"),
    "No other codex connection runs on another account that can start now. Add one with another account in \
     Connections."
  );
  assert!(!page.has("plan-limit-connection"));
  // Waiting is the default: *Continue at …* confirms it, the time alone when the reset is today.
  let continue_at = page.text("plan-limit-continue");
  assert!(continue_at.starts_with("Continue at "), "{continue_at}");
  let wait_detail = page.text("plan-limit-wait");
  assert!(
    wait_detail.contains("Orchester resumes") && !wait_detail.contains("At today"),
    "{wait_detail}"
  );
  assert!(
    !page.locked("plan-limit-continue"),
    "a person's confirmation, not a locked label"
  );
  assert_eq!(page.text("plan-limit-offline"), OFFLINE);
  // Beside *Continue at …* and *Try again now*, the note keeps to one line at 1440.
  assert_eq!(
    page.text("plan-limit-note"),
    "Until then the run stays stopped and holds no agent slot."
  );
  let (note, button) = (page.bounds("plan-limit-note"), page.bounds("plan-limit-continue"));
  assert!(note.height <= button.height, "{note:?} wraps beside {button:?}");
  let accounts = page.text("plan-limit-accounts");
  assert!(
    accounts.contains("account Default · stopped at its plan limit"),
    "{accounts}"
  );
  assert!(page.text("task-status").contains("Stopped at plan limit"));
  assert_eq!(
    page.text("plan-limit-try-now"),
    "Try again now",
    "the same account may be tried now"
  );
  assert!(page.text("task-this-run-account-value").ends_with("limit reached"));
}

#[test]
fn decision_plan_limit_a_new_run_waits_for_its_accounts_reset_before_its_agent_starts() {
  let mut page = Page::with_codex_in("accounts", false);
  page.open("waiting for its account's plan limit", "plan-limit-waiting");
  let title = page.text("plan-limit-waiting-title");
  assert!(
    title.starts_with("Waiting for Default's plan limit · starts at "),
    "{title}"
  );
  let body = page.text("plan-limit-waiting-body");
  assert!(
    body.contains("starts 01 Research") && body.contains("holds no agent slot"),
    "{body}"
  );
  assert!(
    page.has("plan-limit-waiting-none"),
    "no other account's connection to start on"
  );
  assert!(!page.has("plan-limit-choice"), "not a stop of a running agent");
  let status = page.text("task-status");
  assert!(status.contains("Waiting for plan reset"), "{status}");
  assert_eq!(
    page.text("plan-limit-try-now"),
    "Try now on Default",
    "the way out with only Default"
  );
}

#[test]
fn decision_plan_limit_a_waiting_run_offers_start_now_on_another_account() {
  let mut page = Page::with_codex_in("accounts", true);
  page.open("waiting for its account's plan limit", "plan-limit-waiting");
  assert_eq!(page.options("plan-limit-waiting-connection"), ["Codex · Work"]);
  // Work is not among this window's accounts, so the connection names it.
  assert_eq!(page.text("plan-limit-start-now"), "Start now on Codex · Work");
}

#[test]
fn decision_plan_limit_continues_on_another_accounts_connection_only_when_chosen() {
  let mut page = Page::with_codex(true);
  page.open("stopped at its account's plan limit", "plan-limit-choice");
  assert!(!page.has("plan-limit-other-reason"), "another account can start now");
  assert!(!page.has("plan-limit-connection"), "waiting is the default");
  page.click("plan-limit-other");
  page.window.until("the other connection", |window| {
    window.texts("plan-limit-connection").is_some()
  });
  assert_eq!(page.options("plan-limit-connection"), ["Codex · Work"]);
  assert_eq!(page.text("plan-limit-continue"), "Continue now on Codex · Work");
  assert_eq!(page.text("plan-limit-offline"), OFFLINE);
  page.click("plan-limit-wait");
  page.until_says("plan-limit-continue", "Continue at ");
  assert!(!page.has("plan-limit-connection"));
}
