//! Plan limits per account (`XPjSc`): the bar names an account only when the project uses more than one of a CLI,
//! an account at its limit says so in `danger` while the others stay, and a press opens each account's windows.

use std::path::PathBuf;

use orchester_app::shell::status_bar::{AccountLimits, ProjectFacts};
use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::{
    AgentProvider,
    plan_limits::{Blocked, LimitBlock, LimitReading, LimitState, LimitWindow, PlanLimits},
  },
  id::AgentAccountId,
  name::Name,
  time::Timestamp,
};
use orchester_workspace::{runner::state::RunnerState, tasks::data::ProjectTasks};

use super::{Bar, facts, kontur, runner};

const HOUR: i64 = 60 * 60 * 1000;

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
  let provider: AgentProvider = account.cli().provider();
  AccountLimits {
    reading: LimitReading {
      provider,
      account: Some(account.id()),
      at: now,
      limits: PlanLimits {
        windows: vec![window],
        plan: None,
        credits: None,
        blocked: reached.then_some(Blocked {
          reason: LimitBlock::Window,
          window: Some(300),
        }),
      },
    },
    account,
  }
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
  let mut bar = bar(vec![limits(account(AccountCli::ClaudeCode, "Work"), 42.0, false)]);
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
