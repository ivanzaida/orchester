//! Token budgets and time limits as the desktop reads them from its fields.

use std::time::Duration;

use orchester_core::{limits::ProjectLimits, tokens::TokenBudget};
use orchester_desktop::budget::{
  LimitsForm, TIME_LIMIT_CHOICES, budget_error, read_budget, read_minutes, time_limit_choices,
};

fn limits<'a>(max: &'a str, default: &'a str, daily: &'a str, time: &'a str) -> LimitsForm<'a> {
  LimitsForm {
    max_run_budget: max,
    default_run_budget: default,
    daily_budget: daily,
    default_time_limit: time,
    separator: ",",
  }
}

#[test]
fn budget_errors_name_the_example_and_the_maximum() {
  let max = TokenBudget::new(10_000_000).expect("above zero");
  let error = |text| read_budget(text, ",", max).expect_err("invalid");
  let digits = error("1.4M");
  assert_eq!(digits.key, "tokens.errors.digits");
  assert_eq!(digits.args, [("example", "1,400,000".to_owned())]);
  let above = error("12,000,000");
  assert_eq!(above.key, "tokens.errors.aboveMax");
  assert_eq!(above.args, [("max", "10M".to_owned())]);
  assert_eq!(error("").key, "tokens.errors.empty");
  assert_eq!(error("0").key, "tokens.errors.zero");
  assert_eq!(
    budget_error(orchester_core::tokens::BudgetInputError::NotDigits, ".").args,
    [("example", "1.400.000".to_owned())]
  );
}

#[test]
fn budget_minutes_are_whole_and_above_zero() {
  assert_eq!(read_minutes(" 45 "), Ok(Duration::from_secs(45 * 60)));
  assert_eq!(
    read_minutes("").map_err(|error| error.key),
    Err("errors.timeLimitEmpty")
  );
  assert_eq!(
    read_minutes("1.5").map_err(|error| error.key),
    Err("errors.timeLimitInvalid")
  );
  assert_eq!(
    read_minutes("0").map_err(|error| error.key),
    Err("errors.timeLimitZero")
  );
}

#[test]
fn budget_limits_form_reads_every_setting() {
  let read = limits("8,000,000", "2,000,000", "30,000,000", "45")
    .limits()
    .expect("valid limits");
  assert_eq!(read.max_run_budget().get(), 8_000_000);
  assert_eq!(read.default_run_budget().get(), 2_000_000);
  assert_eq!(read.daily_budget().get(), 30_000_000);
  assert_eq!(read.default_time_limit(), Duration::from_secs(45 * 60));
  assert_eq!(
    limits("10,000,000", "1,400,000", "20,000,000", "30").limits(),
    Ok(ProjectLimits::DEFAULT)
  );
}

#[test]
fn budget_limits_form_keeps_the_default_within_the_maximum() {
  let errors = limits("1,000,000", "1,400,000", "20,000,000", "30")
    .limits()
    .expect_err("the default is above the maximum");
  assert_eq!(
    errors.default_run_budget.map(|error| error.key),
    Some("tokens.errors.defaultAboveMax")
  );
  assert_eq!(errors.max_run_budget, None);
  let errors = limits("", "1.4M", "20,000,000", "0")
    .limits()
    .expect_err("invalid fields");
  assert_eq!(
    errors.max_run_budget.map(|error| error.key),
    Some("tokens.errors.empty")
  );
  assert_eq!(
    errors.default_run_budget.map(|error| error.key),
    Some("tokens.errors.digits")
  );
  assert_eq!(errors.daily_budget, None);
  assert_eq!(
    errors.default_time_limit.map(|error| error.key),
    Some("errors.timeLimitZero")
  );
}

#[test]
fn time_limit_choices_include_the_default_and_the_stored_limit() {
  let default = ProjectLimits::DEFAULT.default_time_limit().as_secs() / 60;
  assert!(TIME_LIMIT_CHOICES.contains(&default));
  assert!(
    TIME_LIMIT_CHOICES.len() <= 8,
    "the menu shows 8 options without scrolling"
  );
  assert_eq!(time_limit_choices(Some(30)), TIME_LIMIT_CHOICES);
  assert_eq!(time_limit_choices(None), TIME_LIMIT_CHOICES);
  assert_eq!(time_limit_choices(Some(25)), [15, 25, 30, 45, 60, 90, 120, 180, 240]);
  assert_eq!(time_limit_choices(Some(600)).last(), Some(&600));
}
