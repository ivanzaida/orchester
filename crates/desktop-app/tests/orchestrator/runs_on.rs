//! What the Orchestrator runs on, from the composer's chip (`05 Account and model picker` `v6mWp8`, `05b On an API
//! provider` `AZp3I`): the picker lists the project's sign-in connections by CLI, each account with its plan's 5-hour
//! use, a limit reached and until when, or faded while it is not signed in, and its API-provider connections with
//! their tokens and no plan limit; choosing one publishes the Orchestrator's next version as its settings page does,
//! only once the person presses *Publish vN*.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  account::{
    AccountCheck, AccountCheckResult, AccountCli, AccountFolder, AccountIdentity, AgentAccount, CheckedProgram,
  },
  connection::{
    AgentProvider, ConnectionAccount,
    plan_limits::{Blocked, LimitBlock, LimitState, LimitWindow, PlanLimits},
  },
  id::{AgentAccountId, ConnectionId},
  name::Name,
  orchestrator::OrchestratorSettings,
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::AppDb;

use super::{
  connections,
  harness::{Orchestrator, Setup},
};

/// A Claude Code account named `name` whose CLI said `signed_in` (as `ana@example.com` on the `pro` plan) or signed
/// out, with a Claude Code connection on it; the account and the connection.
fn on_account(screen: &Orchestrator, name: &str, signed_in: bool) -> (AgentAccount, ConnectionId) {
  let folder = screen.window.data.dir.path().join(format!("claude-{name}"));
  std::fs::create_dir_all(&folder).expect("the account's folder");
  let now = Timestamp::now();
  let mut account = AgentAccount::new(
    AgentAccountId::generate(),
    AccountCli::ClaudeCode,
    Name::new(name).expect("a name"),
    AccountFolder::Created(folder.clone()),
    now,
  )
  .expect("an account");
  let program = CheckedProgram {
    path: folder.join("claude.cmd"),
    version: "2.1.89".to_owned(),
  };
  let result = if signed_in {
    AccountCheckResult::SignedIn {
      program,
      identity: AccountIdentity {
        email: Some("ana@example.com".to_owned()),
        plan: Some("pro".to_owned()),
        ..AccountIdentity::default()
      },
    }
  } else {
    AccountCheckResult::SignedOut { program, was: None }
  };
  account.record_check(AccountCheck { at: now, result });
  let mut app = AppDb::open(&screen.window.data.database()).expect("the application database");
  app.add_agent_account(&account).expect("added");
  app.save_agent_account(&account).expect("its check");
  let mut connection = connections::cli(
    AgentProvider::ClaudeCodeCli,
    &format!("Claude · {name}"),
    &folder.join("claude.cmd"),
  );
  connection
    .set_account(ConnectionAccount::of(&account))
    .expect("a Claude Code account");
  // Choosing an account drops the connection's check: it is checked on the account again.
  let checked = connections::cli(AgentProvider::ClaudeCodeCli, "check", &folder.join("claude.cmd"));
  if let Some(check) = checked.last_check() {
    connection.record_check(check.clone()).expect("its check");
  }
  let id = connection.id();
  screen
    .db()
    .create_connection(&connection.into_connection())
    .expect("created");
  (account, id)
}

/// Keeps a read of `account`'s plan: its 5-hour window `used` percent full, reached when `reached`, resetting in two
/// hours.
fn read_plan(screen: &Orchestrator, account: &AgentAccount, (used, reached): (f64, bool)) {
  let now = Timestamp::now();
  let state = if reached {
    LimitState::Reached
  } else {
    LimitState::Normal
  };
  let resets = Timestamp::from_unix_millis(now.unix_millis() + 2 * 60 * 60 * 1000);
  let limits = PlanLimits {
    windows: vec![LimitWindow::new(300, Some(used), Some(resets), state, false).expect("a window")],
    plan: None,
    credits: None,
    blocked: reached.then_some(Blocked {
      reason: LimitBlock::Window,
      window: Some(300),
    }),
  };
  screen
    .window
    .launch
    .workspace()
    .expect("the session started")
    .save_plan_read((account.id(), AccountCli::ClaudeCode), now, &limits)
    .expect("kept");
}

#[test]
fn orchestrator_picker_shows_each_accounts_plan_use_its_limit_and_a_signed_out_one_faded() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let (work, at_work) = on_account(&screen, "Work", true);
  read_plan(&screen, &work, (42.0, false));
  let (busy, at_busy) = on_account(&screen, "Busy", true);
  read_plan(&screen, &busy, (100.0, true));
  let (_, signed_out) = on_account(&screen, "Away", false);
  screen.go(OrchestratorChat::New);
  screen.click("orchestrator-runs-on-chip");
  screen.until("the accounts' plans", |screen| {
    screen
      .text(&format!("orchestrator-runs-on-{at_work}-plan"))
      .contains("42%")
      && screen
        .text(&format!("orchestrator-runs-on-{at_busy}-plan"))
        .contains("limit reached")
  });
  assert_eq!(
    screen.text(&format!("orchestrator-runs-on-{at_work}-plan")),
    "42% of 5h"
  );
  assert!(
    screen.exists(&format!("orchestrator-runs-on-{at_work}-meter")),
    "with its meter"
  );
  let work_row = screen.text(&format!("orchestrator-runs-on-{at_work}"));
  assert!(work_row.contains("ana@example.com · "), "{work_row}");
  let busy_row = screen.text(&format!("orchestrator-runs-on-{at_busy}"));
  assert!(busy_row.contains("ana@example.com · limit until "), "{busy_row}");
  let away = screen.text(&format!("orchestrator-runs-on-{signed_out}"));
  assert!(away.contains("Not signed in · sign in in Connections"), "{away}");
  assert!(
    !screen.exists(&format!("orchestrator-runs-on-{signed_out}-plan")),
    "no plan is read of an account not signed in"
  );
  // A faded row cannot be chosen: pressing it changes nothing.
  screen.press(&format!("orchestrator-runs-on-{signed_out}"));
  assert!(!screen.exists("orchestrator-runs-on-publish"), "nothing chosen");
}

#[test]
fn orchestrator_picker_lists_sign_ins_and_api_providers_and_publishes_the_choice() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let program = std::env::current_exe().expect("a program");
  let deepseek = connections::api_provider("DeepSeek", &program);
  screen
    .db()
    .create_connection(&deepseek)
    .expect("the API provider's connection");
  screen.go(OrchestratorChat::New);
  screen.click("orchestrator-runs-on-chip");
  screen.until("the picker", |screen| screen.exists("orchestrator-runs-on-menu"));
  let menu = screen.text("orchestrator-runs-on-menu");
  for part in [
    "Claude Code · your sign-ins",
    "Codex · your sign-ins",
    "API providers · pay per token",
    "Claude Code · api.deepseek.com",
    "0 today",
    "no plan limit",
    "Another account starts a new session: earlier messages aren't remembered.",
    "Settings ↗",
  ] {
    assert!(menu.contains(part), "{part} in {menu}");
  }
  assert_eq!(
    screen.text(&format!("orchestrator-runs-on-{}-name", deepseek.id())),
    "DeepSeek"
  );
  // What it runs on now is chosen; the effort row offers the connection's efforts, Default first.
  assert_eq!(screen.text("orchestrator-runs-on-model-value"), "haiku");
  assert!(screen.exists("orchestrator-runs-on-effort"));
  assert!(!screen.exists("orchestrator-runs-on-publish"), "nothing changed yet");

  screen.press(&format!("orchestrator-runs-on-{}", deepseek.id()));
  screen.until("the provider chosen", |screen| {
    screen.exists("orchestrator-runs-on-sent-to")
  });
  assert!(screen.text("orchestrator-runs-on-sent-to").contains("api.deepseek.com"));
  assert_eq!(screen.text("orchestrator-runs-on-model-value"), "deepseek-v4-pro");
  assert!(
    !screen.exists("orchestrator-runs-on-effort"),
    "a provider that lists no efforts has no effort row"
  );
  assert_eq!(
    screen
      .db()
      .orchestrator_published()
      .expect("read")
      .map(|(version, _)| version),
    Some(1),
    "choosing alone changes nothing"
  );
  assert_eq!(screen.text("orchestrator-runs-on-publish"), "Publish v2");
  screen.click("orchestrator-runs-on-publish");
  let chosen = OrchestratorSettings::new(deepseek.id(), ModelId::new("deepseek-v4-pro").expect("model"), None);
  screen.until("the next version", |screen| {
    screen.db().orchestrator_published().expect("read") == Some((2, chosen.clone()))
  });
  screen.until("the chip", |screen| {
    screen.text("orchestrator-runs-on") == "DeepSeek · deepseek-v4-pro"
  });
}
