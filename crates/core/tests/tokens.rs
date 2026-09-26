use std::time::Duration;

use orchester_core::{
  limits::{ProjectLimits, ProjectLimitsError},
  run::TokenUsage,
  time::Timestamp,
  tokens::{BudgetInputError, TokenBudget, TokensUsed, grouped, parse_budget, regroup},
};

fn budget(count: u64) -> TokenBudget {
  TokenBudget::new(count).expect("above zero")
}

const MAX: u64 = 10_000_000;

#[test]
fn tokens_budget_input_takes_whole_numbers_with_locale_groups() {
  let parse = |text: &str| parse_budget(text, ",", budget(MAX)).map(TokenBudget::get);
  assert_eq!(parse("1400000"), Ok(1_400_000));
  assert_eq!(parse("1,400,000"), Ok(1_400_000));
  assert_eq!(parse(" 1 400 000 "), Ok(1_400_000));
  assert_eq!(parse("1_400_000"), Ok(1_400_000));
  assert_eq!(parse("1\u{a0}400\u{a0}000"), Ok(1_400_000));
  assert_eq!(parse("10,000,000"), Ok(MAX));
}

#[test]
fn tokens_budget_input_names_each_problem() {
  let parse = |text: &str| parse_budget(text, ",", budget(MAX));
  assert_eq!(parse(""), Err(BudgetInputError::Empty));
  assert_eq!(parse("   "), Err(BudgetInputError::Empty));
  for invalid in ["1.4M", "1.5", "250k", "abc", "-5", "1.400.000", "+3", ","] {
    assert_eq!(parse(invalid), Err(BudgetInputError::NotDigits), "{invalid}");
  }
  assert_eq!(parse("0"), Err(BudgetInputError::Zero));
  assert_eq!(parse("0,000"), Err(BudgetInputError::Zero));
  let above = Err(BudgetInputError::AboveMaximum { max: budget(MAX) });
  assert_eq!(parse("12,000,000"), above);
  assert_eq!(parse("99999999999999999999999"), above);
}

#[test]
fn tokens_budget_input_uses_the_locale_separator() {
  // A locale grouping with dots reads `1.400.000`; a comma is then not a group separator.
  let parse = |text: &str| parse_budget(text, ".", budget(MAX)).map(TokenBudget::get);
  assert_eq!(parse("1.400.000"), Ok(1_400_000));
  assert_eq!(parse("1,4"), Err(BudgetInputError::NotDigits));
}

#[test]
fn tokens_regroup_groups_digits_while_typing() {
  assert_eq!(regroup("1400000", ",").as_deref(), Some("1,400,000"));
  assert_eq!(regroup("1,40000", ",").as_deref(), Some("140,000"));
  assert_eq!(regroup("140", ",").as_deref(), Some("140"));
  assert_eq!(regroup("0012", ",").as_deref(), Some("0,012"));
  assert_eq!(regroup("1.4M", ","), None);
  assert_eq!(regroup("", ","), None);
  assert_eq!(grouped(20_000_000, ","), "20,000,000");
  assert_eq!(grouped(980, ","), "980");
}

#[test]
fn tokens_usage_counts_input_and_output_without_cached_input() {
  let usage = TokenUsage {
    input: 12_000,
    cached_input: 9_000,
    output: 500,
  };
  assert_eq!(usage.counted_tokens(), TokensUsed::new(3_500));
  let used = TokensUsed::new(1_400_000);
  assert!(used.reached(budget(1_400_000)));
  assert!(!used.reached(budget(2_000_000)));
  assert_eq!(used.left_of(budget(2_000_000)), 600_000);
  assert_eq!(TokensUsed::new(1_500_000).left_of(budget(1_400_000)), 0);
}

#[test]
fn tokens_project_limits_default_to_the_design_values() {
  let limits = ProjectLimits::default();
  assert_eq!(limits.max_run_budget().get(), 10_000_000);
  assert_eq!(limits.default_run_budget().get(), 1_400_000);
  assert_eq!(limits.daily_budget().get(), 20_000_000);
  assert_eq!(limits.default_time_limit(), Duration::from_secs(30 * 60));
}

#[test]
fn tokens_project_limits_keep_the_default_within_the_maximum() {
  let thirty = Duration::from_secs(1_800);
  assert_eq!(
    ProjectLimits::new(budget(1_000_000), budget(1_400_000), budget(MAX), thirty),
    Err(ProjectLimitsError::DefaultAboveMaximum {
      default: budget(1_400_000),
      max: budget(1_000_000)
    })
  );
  assert_eq!(
    ProjectLimits::new(budget(MAX), budget(1_400_000), budget(MAX), Duration::ZERO),
    Err(ProjectLimitsError::ZeroTimeLimit)
  );
  assert!(ProjectLimits::new(budget(MAX), budget(MAX), budget(1), thirty).is_ok());
}

#[test]
fn tokens_daily_budget_is_used_once_today_reaches_it() {
  let limits = ProjectLimits::DEFAULT;
  assert!(!limits.daily_budget_used(TokensUsed::new(19_999_999)));
  assert!(limits.daily_budget_used(TokensUsed::new(20_000_000)));
  assert!(limits.daily_budget_used(TokensUsed::new(20_000_001)));
}

#[test]
fn tokens_the_local_day_starts_at_or_before_now_and_within_a_day() {
  let now = Timestamp::now();
  let start = now.local_day_start();
  assert!(start <= now);
  assert!(now.unix_millis() - start.unix_millis() < 25 * 60 * 60 * 1000);
  assert_eq!(start.local_day_start(), start);
}
