//! Token budgets and time limits as the desktop reads them from its fields.

use std::time::Duration;

use orchester_core::{
  limits::{AgentSlots, ProjectLimits},
  tokens::TokenBudget,
};
use orchester_workspace::budget::{
  LimitsForm, TIME_LIMIT_CHOICES, budget_error, read_agent_slots, read_budget, read_minutes, read_optional_budget,
  time_limit_choices,
};

fn limits<'a>(default: &'a str, daily: &'a str, time: &'a str) -> LimitsForm<'a> {
  LimitsForm {
    default_run_budget: default,
    daily_budget: daily,
    default_time_limit: time,
    agent_slots: "4",
    separator: ",",
  }
}

#[test]
fn budget_errors_name_the_example_and_there_is_no_maximum() {
  let error = |text| read_budget(text, ",").expect_err("invalid");
  let digits = error("1.4M");
  assert_eq!(digits.key, "tokens.errors.digits");
  assert_eq!(digits.args, [("example", "1,400,000".to_owned())]);
  assert_eq!(error("99999999999999999999999").key, "tokens.errors.tooLarge");
  assert_eq!(error("").key, "tokens.errors.empty");
  assert_eq!(error("0").key, "tokens.errors.zero");
  assert_eq!(
    read_budget("12,000,000,000", ",").map(TokenBudget::get),
    Ok(12_000_000_000)
  );
  assert_eq!(read_optional_budget(" ", ","), Ok(None));
  assert_eq!(
    read_optional_budget("0", ",").map_err(|error| error.key),
    Err("tokens.errors.zero")
  );
  assert_eq!(
    budget_error(orchester_core::tokens::BudgetInputError::NotDigits, ".").args,
    [("example", "1.400.000".to_owned())]
  );
}

#[test]
fn budget_minutes_are_whole_and_above_zero_or_empty_for_no_limit() {
  assert_eq!(read_minutes(" 45 "), Ok(Some(Duration::from_secs(45 * 60))));
  assert_eq!(read_minutes(""), Ok(None));
  assert_eq!(read_minutes("  "), Ok(None));
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
  let read = limits("2,000,000", "30,000,000", "45").limits().expect("valid limits");
  assert_eq!(read.default_run_budget().map(TokenBudget::get), Some(2_000_000));
  assert_eq!(read.daily_budget().get(), 30_000_000);
  assert_eq!(read.default_time_limit(), Some(Duration::from_secs(45 * 60)));
  assert_eq!(limits("", "20,000,000", "").limits(), Ok(ProjectLimits::DEFAULT));
  // No maximum: a default far above the daily budget is accepted.
  let large = limits("900,000,000", "1", "").limits().expect("valid limits");
  assert_eq!(large.default_run_budget().map(TokenBudget::get), Some(900_000_000));
}

#[test]
fn budget_agent_slots_are_a_whole_number_from_one_to_sixteen() {
  assert_eq!(read_agent_slots(" 16 "), Ok(AgentSlots::new(16).expect("in range")));
  assert_eq!(read_agent_slots("1"), Ok(AgentSlots::new(1).expect("in range")));
  for text in ["", "0", "17", "2.5", "-1", "four"] {
    let error = read_agent_slots(text).expect_err("outside 1 to 16");
    assert_eq!(error.key, "settings.limits.agentSlotsInvalid");
    assert_eq!(error.args, [("min", "1".to_owned()), ("max", "16".to_owned())]);
  }
  let form = LimitsForm {
    agent_slots: "2",
    ..limits("1,400,000", "20,000,000", "30")
  };
  assert_eq!(
    form.limits().map(ProjectLimits::agent_slots),
    Ok(AgentSlots::new(2).expect("in range"))
  );
  let form = LimitsForm {
    agent_slots: "20",
    ..limits("1,400,000", "20,000,000", "30")
  };
  let errors = form.limits().expect_err("too many agents");
  assert_eq!(
    errors.agent_slots.map(|error| error.key),
    Some("settings.limits.agentSlotsInvalid")
  );
  assert_eq!(errors.default_run_budget, None);
}

#[test]
fn budget_limits_form_names_each_malformed_field() {
  let errors = limits("1.4M", "", "0").limits().expect_err("invalid fields");
  assert_eq!(errors.daily_budget.map(|error| error.key), Some("tokens.errors.empty"));
  assert_eq!(
    errors.default_run_budget.map(|error| error.key),
    Some("tokens.errors.digits")
  );
  assert_eq!(
    errors.default_time_limit.map(|error| error.key),
    Some("errors.timeLimitZero")
  );
}

#[test]
fn time_limit_choices_include_the_stored_limit() {
  assert!(
    TIME_LIMIT_CHOICES.len() <= 8,
    "the menu shows 8 options without scrolling"
  );
  assert_eq!(time_limit_choices(Some(30)), TIME_LIMIT_CHOICES);
  assert_eq!(time_limit_choices(None), TIME_LIMIT_CHOICES);
  assert_eq!(time_limit_choices(Some(25)), [15, 25, 30, 45, 60, 90, 120, 180, 240]);
  assert_eq!(time_limit_choices(Some(600)).last(), Some(&600));
}
