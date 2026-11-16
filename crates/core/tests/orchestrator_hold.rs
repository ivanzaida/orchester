//! What holds the turns Orchester starts by itself (`orchestrator::hold`): a used daily budget until the next local
//! day starts, else the plan limit of the Orchestrator's account until the reset its CLI named, or an hour after the
//! report when it named none; a limit that lifted holds nothing.
//! `cargo test -p orchester-core --test orchestrator_hold hold`.

use orchester_core::{
  orchestrator::{Held, PlanHold, UNKNOWN_RESET},
  time::Timestamp,
};

const HOUR: i64 = 60 * 60 * 1000;

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

#[test]
fn hold_of_a_used_daily_budget_lasts_until_the_next_local_day_starts() {
  let now = Timestamp::now();
  let Some(Held::Budget { until }) = Held::at(now, true, None) else {
    panic!("a used budget holds");
  };
  assert!(until > now, "tomorrow");
  assert!(
    until.unix_millis() - now.unix_millis() <= 25 * HOUR,
    "no later than a day on"
  );
  assert_eq!(until.local_day_start(), until, "the start of a local day");
  assert_eq!(
    Held::at(until, true, None).map(Held::until).map(|next| next > until),
    Some(true),
    "the day after holds until the one after it"
  );
}

#[test]
fn hold_of_a_plan_limit_lasts_until_the_reset_its_cli_named() {
  let plan = PlanHold {
    since: at(10 * HOUR),
    resets: Some(at(15 * HOUR)),
  };
  assert_eq!(
    Held::at(at(11 * HOUR), false, Some(plan)),
    Some(Held::PlanLimit { until: at(15 * HOUR) })
  );
  assert_eq!(Held::at(at(15 * HOUR), false, Some(plan)), None, "it lifted");
}

#[test]
fn hold_of_a_plan_limit_without_a_reset_lasts_an_hour_from_the_report() {
  let plan = PlanHold {
    since: at(10 * HOUR),
    resets: None,
  };
  assert_eq!(i64::try_from(UNKNOWN_RESET.as_millis()).expect("an hour"), HOUR);
  assert_eq!(plan.until(), at(11 * HOUR));
  assert!(Held::at(at(11 * HOUR - 1), false, Some(plan)).is_some());
  assert_eq!(
    Held::at(at(11 * HOUR), false, Some(plan)),
    None,
    "the next turn is tried"
  );
}

#[test]
fn hold_names_the_budget_before_the_plan_limit_and_nothing_while_neither_holds() {
  let now = Timestamp::now();
  let plan = PlanHold {
    since: now,
    resets: None,
  };
  assert!(matches!(Held::at(now, true, Some(plan)), Some(Held::Budget { .. })));
  assert_eq!(Held::at(now, false, None), None);
}
