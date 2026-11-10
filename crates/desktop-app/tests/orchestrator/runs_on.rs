//! What the Orchestrator runs on, from the composer's chip (`05 · Account and model` `v6mWp8`, `05b · On DeepSeek`
//! `AZp3I`, `Runs-on picker · states` `t7cdi`): the shared compact picker, 300 wide, with the project's connections on a
//! rail, the one shown on a line under it with its plan's use and limit (an API provider's tokens and no plan limit on
//! a second line), its models and the chosen model's efforts. One not signed in is faded and not to be chosen. An API
//! provider takes a model and an effort of the person's own and says where its data goes. Choosing a connection, a
//! model or an effort publishes the Orchestrator's next version at once, with no *Publish* step, and a refused choice
//! says why and changes nothing.

use std::time::{Duration, Instant};

use orchester_app::{chrome::TITLE_BAR_HEIGHT, routes::OrchestratorChat, ui::tooltip::TOOLTIP_DELAY};
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
  history::kept,
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

/// The picker's element ids start with this.
const MENU: &str = "orchestrator-runs-on-menu";

fn part(name: &str) -> String {
  format!("{MENU}-{name}")
}

/// The rail's item of `connection`.
fn on_rail(connection: ConnectionId) -> String {
  part(&format!("connection-{connection}"))
}

/// Opens the picker from the chip of a new conversation.
fn open(screen: &mut Orchestrator) {
  screen.go(OrchestratorChat::New);
  screen.click("orchestrator-runs-on-chip");
  screen.until("the picker", |screen| screen.exists(MENU));
}

/// The pointer rests on `id` until its tooltip shows.
fn rest_on(screen: &mut Orchestrator, id: &str) {
  let frame = screen.bounds(id);
  let tree = &mut screen.window.tree;
  tree.mouse_move(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
  screen.window.pass();
  screen
    .window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  screen.window.pass();
}

/// The limit dot of `busy`'s item and the bar of `chosen`'s are drawn where the design has them (`V7gbO`, `yTnqv` in
/// `ZcGGy`).
fn assert_marks(screen: &mut Orchestrator, (busy, chosen): (ConnectionId, ConnectionId)) {
  // The dot is 6 at (19, 4) of its 28-square item; the chosen one's bar 12 × 2 at (8, 26).
  let (busy_item, dot) = (screen.bounds(&on_rail(busy)), screen.bounds(&format!("{}-limit", on_rail(busy))));
  assert!(
    (dot.width - 6.0).abs() < 0.5 && (dot.height - 6.0).abs() < 0.5,
    "the dot is drawn: {dot:?}"
  );
  assert!(
    (dot.x - busy_item.x - 19.0).abs() < 0.5 && (dot.y - busy_item.y - 4.0).abs() < 0.5,
    "at its item's top right: {dot:?} {busy_item:?}"
  );
  let (item, bar) = (
    screen.bounds(&on_rail(chosen)),
    screen.bounds(&format!("{}-bar", on_rail(chosen))),
  );
  assert!(
    (bar.width - 12.0).abs() < 0.5 && (bar.height - 2.0).abs() < 0.5,
    "the bar is drawn: {bar:?}"
  );
  assert!(
    (bar.x - item.x - 8.0).abs() < 0.5 && (bar.y - item.y - 26.0).abs() < 0.5,
    "under its item's mark: {bar:?} {item:?}"
  );
}

/// The picker is 300 wide, 6 over its chip and under the title bar, with no foot.
fn assert_frame(screen: &mut Orchestrator) {
  let (menu, chip) = (screen.bounds(MENU), screen.bounds("orchestrator-runs-on-chip"));
  assert!((menu.width - 300.0).abs() < 0.5, "300 wide: {menu:?}");
  assert!(menu.y >= TITLE_BAR_HEIGHT - 0.5, "under the title bar: {menu:?}");
  assert!(
    (chip.y - (menu.y + menu.height) - 6.0).abs() < 1.0,
    "6 over the chip: {menu:?} {chip:?}"
  );
  assert!((menu.x - chip.x).abs() < 1.5, "aligned to the chip's start: {menu:?} {chip:?}");
  assert!(!screen.exists("orchestrator-runs-on-foot"), "no foot");
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
  for id in ["rail-scroll", "search", "model-scroll", "effort-row"] {
    assert!(screen.exists(&part(id)), "{id}");
  }
  for id in [at_work, at_busy, signed_out, screen.claude, screen.codex] {
    assert!(screen.exists(&on_rail(id)), "{id} on the rail");
  }
  // What it runs on is chosen: its bar on the rail, its model checked, its efforts under them, Default first.
  let claude = screen.claude;
  assert!(screen.exists(&format!("{}-bar", on_rail(claude))));
  assert_eq!(screen.text(&part("model-0")), "haiku");
  assert_eq!(screen.text(&part("model-1")), "opus");
  assert_eq!(screen.text(&part("effort-default")), "Default");
  assert_eq!(screen.text(&part("effort-low")), "Low");
  assert!(!screen.exists(&part("publish")), "no Publish step");
  // A plan at its limit has its dot on the rail; one not signed in cannot be chosen.
  screen.until("Busy's limit read", |screen| {
    screen.exists(&format!("{}-limit", on_rail(at_busy)))
  });
  assert_marks(&mut screen, (at_busy, claude));
  assert!(!screen.exists(&format!("{}-limit", on_rail(at_work))));
  screen.press(&on_rail(signed_out));
  assert_eq!(
    published(&screen).map(|(version, _)| version),
    Some(1),
    "nothing chosen"
  );

  // Choosing Work runs on it at once, and its line says its plan and its plan's use, over a 32 meter filled to it.
  screen.press(&on_rail(at_work));
  screen.until("Work published", |screen| {
    published(screen).is_some_and(|(version, settings)| version == 2 && settings.connection() == at_work)
  });
  screen.until("Work's plan", |screen| screen.text(&part("who-use")) == "5h 42%");
  assert_eq!(screen.text(&part("who-name")), "Work");
  assert_eq!(screen.text(&part("who-caption")), "Claude Code · Pro plan");
  let meter = screen.bounds(&part("who-meter"));
  assert!((meter.width - 32.0).abs() < 0.5, "with its meter: {meter:?}");
  let fill = screen.bounds(&part("who-meter-fill"));
  assert!((fill.width - 32.0 * 0.42).abs() < 1.0, "filled to 42%: {fill:?}");
  // Busy's line says until when its limit holds, and the limit fills its meter, both in warning.
  screen.press(&on_rail(at_busy));
  screen.until("Busy's line", |screen| {
    screen.text(&part("who-caption")).starts_with("Claude Code · limit until ")
  });
  assert_eq!(screen.text(&part("who-detail")), "limit reached");
  let fill = screen.bounds(&part("who-meter-fill"));
  assert!((fill.width - 32.0).abs() < 0.5, "the limit fills the meter: {fill:?}");
  assert_frame(&mut screen);
}

#[test]
fn orchestrator_picker_applies_a_model_and_an_effort_at_once() {
  let mut screen = Orchestrator::new(Setup::Ready);
  open(&mut screen);
  screen.press(&part("model-1"));
  let opus = OrchestratorSettings::new(screen.claude, ModelId::new("opus").expect("model"), None);
  screen.until("opus published", |screen| published(screen) == Some((2, opus.clone())));
  screen.until("the chip", |screen| {
    screen.text("orchestrator-runs-on").ends_with("· opus")
  });
  assert!(screen.exists(MENU), "the picker stays open");
  screen.press(&part("effort-high"));
  let high = OrchestratorSettings::new(
    screen.claude,
    ModelId::new("opus").expect("model"),
    Some("high".to_owned()),
  );
  screen.until("high published", |screen| published(screen) == Some((3, high.clone())));
  // Choosing what is chosen publishes nothing.
  screen.press(&part("effort-high"));
  screen.window.pass();
  assert_eq!(published(&screen).map(|(version, _)| version), Some(3));
}

/// A DeepSeek connection of `screen`'s project, chosen in the open picker.
fn on_deepseek(screen: &mut Orchestrator) -> ConnectionId {
  let program = std::env::current_exe().expect("a program");
  let deepseek = connections::api_provider("DeepSeek", &program);
  screen
    .db()
    .create_connection(&deepseek)
    .expect("the API provider's connection");
  open(screen);
  screen.press(&on_rail(deepseek.id()));
  let pro = OrchestratorSettings::new(deepseek.id(), ModelId::new("deepseek-v4-pro").expect("model"), None);
  screen.until("DeepSeek published", |screen| {
    published(screen) == Some((2, pro.clone()))
  });
  deepseek.id()
}

/// Types `text` into the field `id` with the focus in it, and presses Enter.
fn enter(screen: &mut Orchestrator, id: &str, text: &str) {
  screen.click(id);
  screen.set(id, text);
  screen.key(("Enter", "Enter"), false);
}

#[test]
fn orchestrator_picker_on_an_api_provider_takes_a_typed_model() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let deepseek = on_deepseek(&mut screen);
  screen.until("its line", |screen| {
    screen.text(&part("who-caption")) == "Claude Code · api.deepseek.com"
  });
  assert_eq!(screen.text(&part("who-name")), "DeepSeek");
  assert_eq!(screen.text(&part("who-detail")), "0 today · no plan limit");
  assert!(!screen.exists(&part("who-meter")), "no plan limit, no meter");
  assert!(screen.text(&part("note-0")).contains("go to api.deepseek.com under its terms."));
  assert!(
    screen
      .text(&part("search"))
      .contains("Search or type a model…")
  );
  // A model it does not list is typed, and taken with Enter.
  screen.set(&part("search"), "deepseek-v4-flash");
  screen.until("the typed model", |screen| {
    screen.text(&part("typed")) == "Use deepseek-v4-flash"
  });
  screen.key(("Enter", "Enter"), false);
  let flash = OrchestratorSettings::new(deepseek, ModelId::new("deepseek-v4-flash").expect("model"), None);
  screen.until("the typed model published", |screen| {
    published(screen) == Some((3, flash.clone()))
  });
}

#[test]
fn orchestrator_picker_on_an_api_provider_takes_a_typed_effort() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let deepseek = on_deepseek(&mut screen);
  // It lists no efforts: a field for the one it takes (`OR6Jq`), and what that means.
  screen.until("its efforts", |screen| screen.exists(&part("effort-typed")));
  assert!(!screen.exists(&part("effort-default")), "no segments");
  assert_eq!(screen.text(&part("effort-hint")), "The provider lists no efforts. Empty is Default.");
  enter(&mut screen, &part("effort-typed"), " think-hard ");
  let pro = ModelId::new("deepseek-v4-pro").expect("model");
  let typed = OrchestratorSettings::new(deepseek, pro, Some("think-hard".to_owned()));
  screen.until("the typed effort published", |screen| {
    published(screen) == Some((3, typed.clone()))
  });
  // Another model keeps it; nothing typed is *Default*.
  screen.set(&part("search"), "deepseek-v4-flash");
  screen.until("the typed model", |screen| screen.exists(&part("typed")));
  screen.press(&part("typed"));
  let flash = ModelId::new("deepseek-v4-flash").expect("model");
  let kept = OrchestratorSettings::new(deepseek, flash.clone(), Some("think-hard".to_owned()));
  screen.until("the effort kept", |screen| published(screen) == Some((4, kept.clone())));
  enter(&mut screen, &part("effort-typed"), "");
  let default = OrchestratorSettings::new(deepseek, flash, None);
  screen.until("Default published", |screen| {
    published(screen) == Some((5, default.clone()))
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
  screen.press(&on_rail(deepseek.id()));
  screen.until("the refusal", |screen| screen.text(&part("notes")).contains("Not saved: "));
  let refusal = screen.text(&part("notes"));
  assert!(refusal.contains("connection was removed"), "it says why: {refusal}");
  assert_eq!(
    published(&screen).map(|(version, _)| version),
    Some(1),
    "nothing changed"
  );
  let claude = screen.claude;
  assert!(
    screen.exists(&format!("{}-bar", on_rail(claude))),
    "what it runs on stays chosen"
  );
}

#[test]
fn orchestrator_picker_says_why_a_connection_the_keys_choose_cannot_be_chosen() {
  let mut screen = Orchestrator::new(Setup::Ready);
  on_account(&screen, "Away", false);
  open(&mut screen);
  // Shift+Tab takes the keys from the models to the rail, on the connection shown; ← lands on Away before it.
  screen.shift_key(("Tab", "Tab"));
  screen.key(("ArrowLeft", "ArrowLeft"), false);
  screen.key(("Enter", "Enter"), false);
  screen.until("why it cannot be chosen", |screen| {
    screen.text(&part("who-caption")) == "Claude Code · Away: not signed in, sign in in Connections"
  });
  assert_eq!(
    published(&screen).map(|(version, _)| version),
    Some(1),
    "nothing changed"
  );
}

#[test]
fn orchestrator_picker_says_in_tooltips_that_another_account_starts_a_new_session() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let (work, at_work) = on_account(&screen, "Work", true);
  let (_, at_home) = on_account(&screen, "Home", true);
  let chat = kept(&screen, "What runs?", Timestamp::now(), "Nothing yet.");
  let haiku = ModelId::new("haiku").expect("model");
  screen
    .db()
    .set_chat_session(chat, "session-1", (at_work, work.id()))
    .expect("session");
  screen
    .db()
    .publish_orchestrator(&OrchestratorSettings::new(at_work, haiku.clone(), None), Timestamp::now())
    .expect("published");
  screen.go(OrchestratorChat::Chat(chat));
  screen.until("the conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Nothing yet.")
  });
  // On the session's own account the chip says nothing of it.
  rest_on(&mut screen, "orchestrator-runs-on-chip");
  assert_eq!(
    screen.text("orchestrator-runs-on-chip-tooltip"),
    "Acts with your project permissions: every action it takes is checked like the same action in the app."
  );
  screen.click("orchestrator-runs-on-chip");
  screen.until("the picker", |screen| screen.exists(MENU));
  rest_on(&mut screen, &on_rail(at_home));
  assert_eq!(
    screen.text(&format!("{}-tip-tooltip", on_rail(at_home))),
    "Claude Code · Home Another account starts a new session: earlier messages aren't remembered."
  );
  rest_on(&mut screen, &on_rail(at_work));
  assert_eq!(
    screen.text(&format!("{}-tip-tooltip", on_rail(at_work))),
    "Claude Code · Work",
    "the session's own account starts nothing new"
  );
  // After a switch to Home the chip's tooltip carries the same sentence, until the first new message.
  screen.press(&on_rail(at_home));
  screen.until("Home published", |screen| {
    published(screen).is_some_and(|(_, settings)| settings.connection() == at_home)
  });
  screen.press("orchestrator-runs-on-chip");
  screen.until("the picker closed", |screen| !screen.exists(MENU));
  screen.until("the chip on Home", |screen| screen.text("orchestrator-runs-on").contains("Home"));
  // The pointer leaves the chip and comes back, as a person would.
  screen.window.tree.mouse_move(0.0, 0.0);
  screen.window.pass();
  rest_on(&mut screen, "orchestrator-runs-on-chip");
  assert!(
    screen
      .text("orchestrator-runs-on-chip-tooltip")
      .ends_with("Another account starts a new session: earlier messages aren't remembered."),
    "{}",
    screen.text("orchestrator-runs-on-chip-tooltip")
  );
}

#[test]
fn orchestrator_picker_without_connections_is_its_foot_with_no_line_over_it() {
  let mut screen = Orchestrator::new(Setup::NoConnection);
  open(&mut screen);
  for part in ["rail-scroll", "search", "model-scroll", "effort-row"] {
    assert!(!screen.exists(&self::part(part)), "no {part}");
  }
  assert_eq!(
    screen.text(MENU),
    "Another account starts a new session: earlier messages aren't remembered. Settings ↗"
  );
  let (menu, foot) = (screen.bounds(MENU), screen.bounds("orchestrator-runs-on-foot"));
  assert!((foot.y - menu.y).abs() < 1.5, "the foot at the top: {menu:?} {foot:?}");
  assert!(
    (menu.width - 360.0).abs() < 0.5,
    "only as wide as its foot (`cjnbn`): {menu:?}"
  );
}
