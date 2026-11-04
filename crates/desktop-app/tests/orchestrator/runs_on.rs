//! What the Orchestrator runs on, from the composer's chip (`05 · Account and model` `v6mWp8`, `05b · On DeepSeek`
//! `AZp3I`, `Runs-on picker · states` `t7cdi`): one panel with the project's connections on a rail, the chosen one's
//! models and the chosen model's efforts; an account's plan use and limit in the head of its models, one not signed in
//! faded and not to be chosen; an API provider with its tokens, no plan limit and no efforts. Choosing a connection, a
//! model or an effort publishes the Orchestrator's next version at once, with no *Publish* step, and a refused choice
//! says why and changes nothing.

use orchester_app::{chrome::TITLE_BAR_HEIGHT, routes::OrchestratorChat};
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

/// What is published now: its version and settings.
fn published(screen: &Orchestrator) -> Option<(u32, OrchestratorSettings)> {
  screen.db().orchestrator_published().expect("read")
}

/// Opens the picker from the chip of a new conversation.
fn open(screen: &mut Orchestrator) {
  screen.go(OrchestratorChat::New);
  screen.click("orchestrator-runs-on-chip");
  screen.until("the picker", |screen| screen.exists("orchestrator-runs-on-menu"));
}

#[test]
fn orchestrator_picker_shows_a_cli_connection_on_the_rail_with_its_models_and_efforts() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let (work, at_work) = on_account(&screen, "Work", true);
  read_plan(&screen, &work, (42.0, false));
  let (busy, at_busy) = on_account(&screen, "Busy", true);
  read_plan(&screen, &busy, (100.0, true));
  let (_, signed_out) = on_account(&screen, "Away", false);
  open(&mut screen);
  for column in [
    "orchestrator-runs-on-rail",
    "orchestrator-runs-on-models",
    "orchestrator-runs-on-effort",
    "orchestrator-runs-on-search",
  ] {
    assert!(screen.exists(column), "{column}");
  }
  for id in [at_work, at_busy, signed_out, screen.claude, screen.codex] {
    assert!(screen.exists(&format!("orchestrator-runs-on-{id}")), "{id} on the rail");
  }
  // What it runs on is chosen: its bar on the rail, its model checked, its efforts beside them, Default first.
  let claude = screen.claude;
  assert!(screen.exists(&format!("orchestrator-runs-on-{claude}-bar")));
  assert_eq!(screen.text("orchestrator-runs-on-model-0"), "haiku");
  assert_eq!(screen.text("orchestrator-runs-on-model-1"), "opus");
  assert_eq!(screen.text("orchestrator-runs-on-effort-model"), "haiku");
  assert_eq!(screen.text("orchestrator-runs-on-effort-0"), "Default the CLI's own");
  assert_eq!(screen.text("orchestrator-runs-on-effort-1"), "Low");
  assert!(!screen.exists("orchestrator-runs-on-publish"), "no Publish step");
  assert!(screen.exists("orchestrator-runs-on-rule"), "a line over the foot");
  // A plan at its limit has its dot on the rail; one not signed in cannot be chosen.
  screen.until("Busy's limit read", |screen| {
    screen.exists(&format!("orchestrator-runs-on-{at_busy}-limit"))
  });
  assert!(!screen.exists(&format!("orchestrator-runs-on-{at_work}-limit")));
  screen.press(&format!("orchestrator-runs-on-{signed_out}"));
  assert_eq!(
    published(&screen).map(|(version, _)| version),
    Some(1),
    "nothing chosen"
  );

  // Choosing Work runs on it at once, and its head says its plan's use over an 88 meter filled to it.
  screen.press(&format!("orchestrator-runs-on-{at_work}"));
  screen.until("Work published", |screen| {
    published(screen).is_some_and(|(version, settings)| version == 2 && settings.connection() == at_work)
  });
  screen.until("Work's plan", |screen| {
    screen.text(&format!("orchestrator-runs-on-{at_work}-plan")) == "42% of 5h"
  });
  assert_eq!(screen.text(&format!("orchestrator-runs-on-{at_work}-name")), "Work");
  assert_eq!(
    screen.text("orchestrator-runs-on-who"),
    "Claude Code · ana@example.com · Pro plan"
  );
  let meter = screen.bounds(&format!("orchestrator-runs-on-{at_work}-meter"));
  assert!((meter.width - 88.0).abs() < 0.5, "with its meter: {meter:?}");
  let fill = screen.bounds(&format!("orchestrator-runs-on-{at_work}-meter-fill"));
  assert!((fill.width - 88.0 * 0.42).abs() < 1.0, "filled to 42%: {fill:?}");
  // Busy's head says until when its limit holds, and the limit fills its meter.
  screen.press(&format!("orchestrator-runs-on-{at_busy}"));
  screen.until("Busy's head", |screen| {
    screen
      .text("orchestrator-runs-on-who")
      .starts_with("Claude Code · ana@example.com · limit until ")
  });
  assert_eq!(
    screen.text(&format!("orchestrator-runs-on-{at_busy}-plan")),
    "limit reached"
  );
  // The picker is 640 wide, over its chip and under the title bar.
  let (menu, chip) = (
    screen.bounds("orchestrator-runs-on-menu"),
    screen.bounds("orchestrator-runs-on-chip"),
  );
  assert!((menu.width - 640.0).abs() < 0.5, "640 wide: {menu:?}");
  assert!(menu.y >= TITLE_BAR_HEIGHT - 0.5, "under the title bar: {menu:?}");
  assert!(menu.y + menu.height <= chip.y, "over the chip: {menu:?} {chip:?}");
  let effort = screen.bounds("orchestrator-runs-on-effort");
  assert!((effort.width - 196.0).abs() < 0.5, "Effort 196 wide: {effort:?}");
}

#[test]
fn orchestrator_picker_applies_a_model_and_an_effort_at_once() {
  let mut screen = Orchestrator::new(Setup::Ready);
  open(&mut screen);
  screen.press("orchestrator-runs-on-model-1");
  let opus = OrchestratorSettings::new(screen.claude, ModelId::new("opus").expect("model"), None);
  screen.until("opus published", |screen| published(screen) == Some((2, opus.clone())));
  screen.until("the chip", |screen| {
    screen.text("orchestrator-runs-on").ends_with("· opus")
  });
  assert!(screen.exists("orchestrator-runs-on-menu"), "the picker stays open");
  screen.until("opus's efforts", |screen| {
    screen.text("orchestrator-runs-on-effort-model") == "opus"
  });
  screen.press("orchestrator-runs-on-effort-2");
  let high = OrchestratorSettings::new(
    screen.claude,
    ModelId::new("opus").expect("model"),
    Some("high".to_owned()),
  );
  screen.until("high published", |screen| published(screen) == Some((3, high.clone())));
  // Choosing what is chosen publishes nothing.
  screen.press("orchestrator-runs-on-effort-2");
  screen.window.pass();
  assert_eq!(published(&screen).map(|(version, _)| version), Some(3));
}

#[test]
fn orchestrator_picker_on_an_api_provider_has_no_efforts_and_takes_a_typed_model() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let program = std::env::current_exe().expect("a program");
  let deepseek = connections::api_provider("DeepSeek", &program);
  screen
    .db()
    .create_connection(&deepseek)
    .expect("the API provider's connection");
  open(&mut screen);
  screen.press(&format!("orchestrator-runs-on-{}", deepseek.id()));
  let pro = OrchestratorSettings::new(deepseek.id(), ModelId::new("deepseek-v4-pro").expect("model"), None);
  screen.until("DeepSeek published", |screen| {
    published(screen) == Some((2, pro.clone()))
  });
  screen.until("its head", |screen| {
    screen.text("orchestrator-runs-on-who") == "Claude Code · api.deepseek.com"
  });
  let usage = screen.text(&format!("orchestrator-runs-on-{}-usage", deepseek.id()));
  assert_eq!(usage, "0 today no plan limit");
  assert!(screen.text("orchestrator-runs-on-sent-to").contains("api.deepseek.com"));
  assert!(
    !screen.exists("orchestrator-runs-on-effort"),
    "no efforts listed: no column"
  );
  assert!(
    screen
      .text("orchestrator-runs-on-search")
      .contains("Search or type a model…")
  );
  // A model it does not list is typed, and taken with Enter.
  screen.set("orchestrator-runs-on-search", "deepseek-v4-flash");
  screen.until("the typed model", |screen| {
    screen.text("orchestrator-runs-on-typed") == "Use deepseek-v4-flash"
  });
  screen.key(("Enter", "Enter"), false);
  let flash = OrchestratorSettings::new(deepseek.id(), ModelId::new("deepseek-v4-flash").expect("model"), None);
  screen.until("the typed model published", |screen| {
    published(screen) == Some((3, flash.clone()))
  });
}

#[test]
fn orchestrator_picker_says_why_a_choice_was_refused_and_changes_nothing() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let program = std::env::current_exe().expect("a program");
  let deepseek = connections::api_provider("DeepSeek", &program);
  screen
    .db()
    .create_connection(&deepseek)
    .expect("the API provider's connection");
  open(&mut screen);
  // Removed after the picker read it: choosing it is refused.
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("the project's database");
  db.execute("DELETE FROM connections WHERE id = ?1", [deepseek.id().to_string()])
    .expect("removed");
  screen.press(&format!("orchestrator-runs-on-{}", deepseek.id()));
  screen.until("the refusal", |screen| screen.exists("orchestrator-runs-on-error"));
  let refusal = screen.text("orchestrator-runs-on-error");
  assert!(!refusal.is_empty(), "it says why");
  assert_eq!(
    published(&screen).map(|(version, _)| version),
    Some(1),
    "nothing changed"
  );
  let claude = screen.claude;
  assert!(
    screen.exists(&format!("orchestrator-runs-on-{claude}-bar")),
    "what it runs on stays chosen"
  );
}

#[test]
fn orchestrator_picker_without_connections_is_its_foot_with_no_line_over_it() {
  let mut screen = Orchestrator::new(Setup::NoConnection);
  open(&mut screen);
  for part in [
    "orchestrator-runs-on-body",
    "orchestrator-runs-on-rail",
    "orchestrator-runs-on-models",
  ] {
    assert!(!screen.exists(part), "no {part}");
  }
  assert_eq!(
    screen.text("orchestrator-runs-on-menu"),
    "Another account starts a new session: earlier messages aren't remembered. Settings ↗"
  );
  assert!(!screen.exists("orchestrator-runs-on-rule"), "no line over the foot");
  let (menu, foot) = (
    screen.bounds("orchestrator-runs-on-menu"),
    screen.bounds("orchestrator-runs-on-foot"),
  );
  assert!((foot.y - menu.y).abs() < 1.5, "the foot at the top: {menu:?} {foot:?}");
}
