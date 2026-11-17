//! `continue_run` of a team task's run (OA-34): decided as its task page offers it, and a raise of its limits asked of
//! the team server first, within the ceiling its Admin set in the workspace policy; with none, the limits stay the
//! Admin's (`decision_required`, D6).

use std::time::Duration;

use orchester_api::v2::LimitRaises;
use orchester_core::{
  run::{RaisedLimits, RunLimit},
  tokens::TokenBudget,
};
use serde_json::{Value, json};

#[test]
fn team_mcp_continue_run_of_a_team_run_raises_its_limits_within_its_admins_ceiling() {
  let team = crate::team_runs::started();
  let run = team.run.to_string();

  // The run works: its page offers no resume, and that is the refusal, not one for being a team run's.
  let resume = json!({"project": "Kontur", "run": run, "choice": {"kind": "resume"}});
  assert_eq!(team.checkout.refused("continue_run", &resume), "not_stopped_at_limit");

  // The run had a budget of 1000 tokens and stopped at it.
  team.settle(|run, now| {
    let budget = TokenBudget::new(1000).expect("a budget");
    let mut events = run.stop_at_limit(RunLimit::Tokens, now).expect("stopped");
    let limits = RaisedLimits {
      budget: Some(budget),
      max_duration: None,
    };
    events.extend(
      run
        .continue_with_limits(limits, Duration::ZERO, now)
        .expect("given a budget"),
    );
    events.extend(run.stop_at_limit(RunLimit::Tokens, now).expect("stopped again"));
    events
  });
  let raise = |tokens: Value| {
    json!({"project": "Kontur", "run": run,
      "choice": {"kind": "raise_limits", "budget_tokens": tokens, "time_limit_minutes": null}})
  };
  // Automation without the person's consent gets the person-only refusal first.
  assert_eq!(
    team.checkout.refused("continue_run", &raise(json!(1200))),
    "automation_not_allowed"
  );
  team
    .checkout
    .workspace
    .set_automation_answers_approvals(true)
    .expect("the switch");

  // The Admin set no ceiling: the limits are the Admin's, and nothing is raised.
  assert_eq!(
    team.checkout.refused("continue_run", &raise(json!(1200))),
    "decision_required"
  );

  // One raise of a run, by at most half: more is refused by the server, a lifted limit before it is asked.
  team.server.state().limit_raises = Some(LimitRaises::new(1, 50).expect("a ceiling"));
  assert_eq!(
    team.checkout.refused("continue_run", &raise(json!(2000))),
    "raise_beyond_ceiling"
  );
  assert_eq!(
    team.checkout.refused("continue_run", &raise(Value::Null)),
    "raise_beyond_ceiling"
  );
  assert!(
    team.server.state().raises.is_empty(),
    "nothing raised beyond the ceiling"
  );

  // Within it, the server keeps the raise; this session has no runner to raise the run with.
  let within = team.checkout.refused("continue_run", &raise(json!(1500)));
  assert_ne!(within, "decision_required");
  assert_eq!(team.server.state().raises.len(), 1, "the server kept the raise");

  // A second raise of the same run is one more than the ceiling allows.
  assert_eq!(
    team.checkout.refused("continue_run", &raise(json!(1400))),
    "raise_beyond_ceiling"
  );
}
