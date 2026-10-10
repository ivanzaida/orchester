use std::time::Duration;

use orchester_core::{
  limits::{AgentSlots, AgentSlotsError, ProjectLimits, ProjectLimitsError},
  run::TokenUsage,
  time::Timestamp,
  tokens::{BudgetInputError, TokenBudget, TokensUsed, grouped, parse_budget, parse_optional_budget, regroup},
};

fn budget(count: u64) -> TokenBudget {
  TokenBudget::new(count).expect("above zero")
}

#[test]
fn tokens_budget_input_takes_whole_numbers_with_locale_groups() {
  let parse = |text: &str| parse_budget(text, ",").map(TokenBudget::get);
  assert_eq!(parse("1400000"), Ok(1_400_000));
  assert_eq!(parse("1,400,000"), Ok(1_400_000));
  assert_eq!(parse(" 1 400 000 "), Ok(1_400_000));
  assert_eq!(parse("1_400_000"), Ok(1_400_000));
  assert_eq!(parse("1\u{a0}400\u{a0}000"), Ok(1_400_000));
  // No maximum: any whole number of tokens a u64 holds.
  assert_eq!(parse("10,000,000"), Ok(10_000_000));
  assert_eq!(parse("900,000,000,000"), Ok(900_000_000_000));
  assert_eq!(parse("9223372036854775807"), Ok(TokenBudget::LARGEST));
  assert_eq!(parse("9223372036854775808"), Err(BudgetInputError::TooLarge));
}

#[test]
fn tokens_optional_budget_input_reads_empty_as_no_limit() {
  let parse = |text: &str| parse_optional_budget(text, ",").map(|budget| budget.map(TokenBudget::get));
  assert_eq!(parse(""), Ok(None));
  assert_eq!(parse("   "), Ok(None));
  assert_eq!(parse("1,400,000"), Ok(Some(1_400_000)));
  assert_eq!(parse("0"), Err(BudgetInputError::Zero));
  assert_eq!(parse("1.4M"), Err(BudgetInputError::NotDigits));
}

#[test]
fn tokens_budget_input_names_each_problem() {
  let parse = |text: &str| parse_budget(text, ",");
  assert_eq!(parse(""), Err(BudgetInputError::Empty));
  assert_eq!(parse("   "), Err(BudgetInputError::Empty));
  for invalid in ["1.4M", "1.5", "250k", "abc", "-5", "1.400.000", "+3", ","] {
    assert_eq!(parse(invalid), Err(BudgetInputError::NotDigits), "{invalid}");
  }
  assert_eq!(parse("0"), Err(BudgetInputError::Zero));
  assert_eq!(parse("0,000"), Err(BudgetInputError::Zero));
  assert_eq!(parse("99999999999999999999999"), Err(BudgetInputError::TooLarge));
}

#[test]
fn tokens_budget_input_uses_the_locale_separator() {
  // A locale grouping with dots reads `1.400.000`; a comma is then not a group separator.
  let parse = |text: &str| parse_budget(text, ".").map(TokenBudget::get);
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
fn tokens_project_limits_default_to_no_run_limits() {
  let limits = ProjectLimits::default();
  assert_eq!(limits.default_run_budget(), None);
  assert_eq!(limits.daily_budget().get(), 20_000_000);
  assert_eq!(limits.default_time_limit(), None);
  assert_eq!(limits.agent_slots().get().get(), 4);
}

#[test]
fn tokens_agent_slots_are_one_to_sixteen() {
  for count in [1, 4, 16] {
    assert_eq!(
      AgentSlots::new(count).map(|slots| u64::from(slots.get().get())),
      Ok(count)
    );
  }
  for count in [0, 17, u64::from(u32::MAX) + 1] {
    assert_eq!(AgentSlots::new(count), Err(AgentSlotsError { count }));
  }
  let two = AgentSlots::new(2).expect("in range");
  let limits = ProjectLimits::DEFAULT.with_agent_slots(two);
  assert_eq!(limits.agent_slots(), two);
  assert_eq!(limits.daily_budget(), ProjectLimits::DEFAULT.daily_budget());
}

#[test]
fn tokens_project_limits_have_optional_defaults_without_a_maximum() {
  let thirty = Some(Duration::from_secs(1_800));
  // A default run budget far above the daily budget is accepted: nothing bounds it.
  let large = ProjectLimits::new(Some(budget(900_000_000)), budget(1), thirty).expect("valid");
  assert_eq!(large.default_run_budget(), Some(budget(900_000_000)));
  let none = ProjectLimits::new(None, budget(20_000_000), None).expect("valid");
  assert_eq!((none.default_run_budget(), none.default_time_limit()), (None, None));
  assert_eq!(
    ProjectLimits::new(None, budget(20_000_000), Some(Duration::ZERO)),
    Err(ProjectLimitsError::ZeroTimeLimit)
  );
}

#[test]
fn tokens_daily_budget_is_used_once_today_reaches_it() {
  let limits = ProjectLimits::DEFAULT;
  assert!(!limits.daily_budget_used(TokensUsed::new(19_999_999)));
  assert!(limits.daily_budget_used(TokensUsed::new(20_000_000)));
  assert!(limits.daily_budget_used(TokensUsed::new(20_000_001)));
}

#[test]
fn tokens_optional_daily_budget_never_stops_work_until_a_limit_is_restored() {
  let unlimited = ProjectLimits::DEFAULT.with_daily_budget_limit(None);
  assert_eq!(unlimited.daily_budget_limit(), None);
  assert!(!unlimited.daily_budget_used(TokensUsed::new(u64::MAX)));
  let restored = unlimited.with_daily_budget_limit(Some(budget(900_000_000)));
  assert!(!restored.daily_budget_used(TokensUsed::new(899_999_999)));
  assert!(restored.daily_budget_used(TokensUsed::new(900_000_000)));
}

#[test]
fn tokens_the_local_day_starts_at_or_before_now_and_within_a_day() {
  let now = Timestamp::now();
  let start = now.local_day_start();
  assert!(start <= now);
  assert!(now.unix_millis() - start.unix_millis() < 25 * 60 * 60 * 1000);
  assert_eq!(start.local_day_start(), start);
}
