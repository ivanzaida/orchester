//! Plan limits per account (`XPjSc`): the bar names an account only when the project uses more than one of a CLI,
//! an account at its limit says so in `danger` while the others stay, and a press opens each account's windows, with
//! how old each account's reading is and whether a turn reported it or Orchester read it.

use std::path::PathBuf;

use orchester_app::shell::status_bar::{AccountLimits, ProjectFacts};
use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::{
    AgentProvider,
    plan_limits::{Blocked, LimitBlock, LimitReading, LimitState, LimitWindow, PlanLimits, ReadingSource},
  },
  id::AgentAccountId,
  name::Name,
  time::Timestamp,
};
use orchester_workspace::{runner::state::RunnerState, tasks::data::ProjectTasks};

use super::{Bar, facts, kontur, runner};

const HOUR: i64 = 60 * 60 * 1000;
const MINUTE: i64 = 60 * 1000;

fn account(cli: AccountCli, name: &str) -> AgentAccount {
  AgentAccount::new(
    AgentAccountId::generate(),
    cli,
    Name::new(name).expect("a name"),
    AccountFolder::Created(PathBuf::from(if cfg!(windows) { r"C:\accounts\a" } else { "/accounts/a" }).join(name)),
    Timestamp::now(),
  )
  .expect("an account")
}

fn limits(account: AgentAccount, used: f64, reached: bool) -> AccountLimits {
  let now = Timestamp::now();
  let state = if reached {
    LimitState::Reached
  } else {
    LimitState::Normal
  };
  let window = LimitWindow::new(
    300,
    Some(used),
    Some(Timestamp::from_unix_millis(now.unix_millis() + 2 * HOUR)),
    state,
    false,
  )
  .expect("a window");
  let mut limits = reading(account, (now, ReadingSource::Turn), vec![window]);
  limits.reading.limits.blocked = reached.then_some(Blocked {
    reason: LimitBlock::Window,
    window: Some(300),
  });
  limits
}

/// `account`'s reading of `windows` at `at`, from `source`.
fn reading(
  account: AgentAccount,
  (at, source): (Timestamp, ReadingSource),
  windows: Vec<LimitWindow>,
) -> AccountLimits {
  let provider: AgentProvider = account.cli().provider();
  AccountLimits {
    reading: LimitReading {
      provider,
      account: Some(account.id()),
      at,
      limits: PlanLimits {
        windows,
        plan: None,
        credits: None,
        blocked: None,
      },
    },
    account,
    source,
  }
}

/// A window of `minutes`, `used` percent full, that resets `resets` from now (in the past when negative).
fn window(minutes: u64, used: f64, resets: i64) -> LimitWindow {
  let at = Timestamp::from_unix_millis(Timestamp::now().unix_millis() + resets);
  LimitWindow::new(minutes, Some(used), Some(at), LimitState::Normal, false).expect("a window")
}

fn bar(limits: Vec<AccountLimits>) -> Bar {
  Bar::of(facts(Some(ProjectFacts {
    limits,
    ..kontur(runner(RunnerState::Online), ProjectTasks::default())
  })))
}

#[test]
fn status_plan_limits_name_an_account_only_where_a_cli_has_more_than_one() {
  let mut both = bar(vec![
    limits(account(AccountCli::ClaudeCode, "Work"), 42.0, false),
    limits(account(AccountCli::ClaudeCode, "Personal"), 8.0, false),
    limits(account(AccountCli::Codex, "Client"), 65.0, false),
  ]);
  let words = both.texts("status-plan-limits").expect("the item shows").join(" ");
  assert_eq!(words, "Claude · Work 42% · Personal 8% · Codex 65%");
  let mut none = bar(Vec::new());
  assert!(none.texts("status-plan-limits").is_none(), "no reading, no item");
}

#[test]
fn status_plan_limits_say_which_account_reached_its_limit_and_open_its_windows() {
  let work = account(AccountCli::ClaudeCode, "Work");
  let mut bar = bar(vec![
    limits(work.clone(), 100.0, true),
    limits(account(AccountCli::ClaudeCode, "Personal"), 9.0, false),
  ]);
  let words = bar.texts("status-plan-limits").expect("the item shows").join(" ");
  assert!(words.starts_with("Claude · Work limit reached · resets "), "{words}");
  assert!(words.ends_with("Personal 9%"), "{words}");
  bar.press("status-plan-limits");
  let windows = bar
    .texts(&format!("status-plan-limits-account-{}", work.id()))
    .expect("Work's windows show")
    .join(" ");
  assert!(
    windows.contains("Claude · Work") && windows.contains("5-hour") && windows.contains("100%"),
    "{windows}"
  );
}

/// lurq 0.46 switches dropdowns in one press when the pressed item is the open popup's anchor: the bar's
/// popovers are anchored to their items, declared while closed too, and still open at the bar's end (`QqBCj`:
/// 9 from the bar, their end 12 from the bar's).
#[test]
fn status_plan_limits_and_mcp_popovers_switch_in_one_press() {
  // A QA copy's server shows its item while it listens; the installed app's is quiet.
  let mut facts = facts(Some(ProjectFacts {
    limits: vec![limits(account(AccountCli::ClaudeCode, "Work"), 42.0, false)],
    ..kontur(runner(RunnerState::Online), ProjectTasks::default())
  }));
  facts.mcp.kind = orchester_app::mcp::McpKind::Qa {
    folder: PathBuf::from(".tmp/qa/orc-242/mcp"),
    file: None,
  };
  let mut bar = Bar::of(facts);
  bar.press("status-plan-limits");
  assert!(bar.texts("status-plan-limits-popover").is_some(), "plan limits open");
  bar.press("status-mcp");
  assert!(
    bar.texts("status-plan-limits-popover").is_none(),
    "the press closes plan limits"
  );
  assert!(bar.texts("status-mcp-popover").is_some(), "and opens MCP");
  let (edge, popover) = (bar.bounds("status-bar"), bar.bounds("status-mcp-popover"));
  let end = (edge.x + edge.width) - (popover.x + popover.width);
  // The bar is alone at the window's top here, so lurq flips the popover under it, 9 from the bar as over it.
  let under = popover.y - (edge.y + edge.height);
  assert!(
    (end - 12.0).abs() < 0.5 && (under - 9.0).abs() < 0.5,
    "end {end}, under {under}"
  );
  bar.press("status-plan-limits");
  assert!(bar.texts("status-mcp-popover").is_none(), "back: MCP closes");
  assert!(
    bar.texts("status-plan-limits-popover").is_some(),
    "and plan limits open"
  );
}

/// Nothing is estimated between readings: the hint says how old each account's reading is and where it came from
/// (*as of 2 min ago · read*, *as of 3 h ago · from a turn*).
#[test]
fn status_plan_limits_hint_says_how_old_each_reading_is_and_where_it_came_from() {
  let now = Timestamp::now().unix_millis();
  let (work, client) = (
    account(AccountCli::ClaudeCode, "Work"),
    account(AccountCli::Codex, "Client"),
  );
  let mut bar = bar(vec![
    reading(
      work.clone(),
      (Timestamp::from_unix_millis(now - 3 * HOUR), ReadingSource::Turn),
      vec![window(10_080, 30.0, 24 * HOUR)],
    ),
    reading(
      client.clone(),
      (Timestamp::from_unix_millis(now - 2 * MINUTE), ReadingSource::Read),
      vec![window(300, 65.0, 2 * HOUR)],
    ),
  ]);
  bar.press("status-plan-limits");
  let as_of = |bar: &mut Bar, account: &AgentAccount| {
    bar
      .texts(&format!("status-plan-limits-as-of-{}", account.id()))
      .expect("the reading's age shows")
      .join(" ")
  };
  assert_eq!(as_of(&mut bar, &work), "as of 3 h ago · from a turn");
  assert_eq!(as_of(&mut bar, &client), "as of 2 min ago · read");
}

/// A window whose reset has passed since its reading is left out rather than shown as still used: its use now is
/// not known until the next reading, and the bar's share is the window that still stands.
#[test]
fn status_plan_limits_leave_out_a_window_reset_since_its_reading() {
  let now = Timestamp::now().unix_millis();
  let client = account(AccountCli::Codex, "Client");
  let mut bar = bar(vec![reading(
    client.clone(),
    (Timestamp::from_unix_millis(now - 6 * HOUR), ReadingSource::Read),
    vec![window(300, 97.0, -HOUR), window(10_080, 41.0, 48 * HOUR)],
  )]);
  let words = bar.texts("status-plan-limits").expect("the item shows").join(" ");
  assert_eq!(words, "Codex 41%", "the reset 5-hour window's 97% is not shown");
  bar.press("status-plan-limits");
  let block = bar
    .texts(&format!("status-plan-limits-account-{}", client.id()))
    .expect("Client's windows show")
    .join(" ");
  assert!(block.contains("Weekly") && block.contains("41%"), "{block}");
  assert!(!block.contains("5-hour") && !block.contains("97%"), "{block}");
  assert!(block.contains("as of 6 h ago · read"), "{block}");
}
