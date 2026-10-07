//! `continue_run` on the real runner and the fake Claude: a run stopped at its token budget continues
//! with the budget and time limit the caller gives, read with the task page's rules; a choice the page
//! does not offer, a budget not above what the run used, and a run not stopped are refused. Run with
//! `cargo test -p orchester-desktop --test form_keyboard app_mcp_continue_run`.

use serde_json::json;

use super::{agent, ok, refused};
use crate::{
  harness::Desktop,
  task_run::{create_and_run, has, init_repository, programs, run_finished, seed_claude},
};

#[test]
fn app_mcp_continue_run_raises_the_limits_of_a_run_stopped_at_them() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| seed_claude(repo, &fake));
  create_and_run(&mut desktop, "Work in turns.", "5000");
  desktop.wait_until("the raise-limit panel", |desktop| has(desktop, "raise-limit"));
  let mut client = agent(&desktop);
  let with = |choice: serde_json::Value| json!({"project": "Kontur", "run": "#1", "choice": choice});

  let retry = refused(&mut client, "continue_run", &with(json!({"kind": "allow_retry"})));
  assert_eq!(retry["code"], "choice_not_offered");
  let low = refused(
    &mut client,
    "continue_run",
    &with(json!({"kind": "raise_limits", "budget_tokens": 5000, "time_limit_minutes": 60})),
  );
  assert_eq!(low["code"], "invalid_limit");
  let missing = refused(&mut client, "continue_run", &with(json!({"kind": "raise_limits"})));
  assert_eq!(
    missing["code"], "invalid_arguments",
    "nothing defaults to a higher limit"
  );

  let continued = ok(
    &mut client,
    "continue_run",
    &with(json!({"kind": "raise_limits", "budget_tokens": 100_000, "time_limit_minutes": 60})),
  );
  assert_ne!(continued["run"]["status"]["state"], "paused", "{continued}");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  let ended = refused(
    &mut client,
    "continue_run",
    &with(json!({"kind": "raise_limits", "budget_tokens": 200_000, "time_limit_minutes": 60})),
  );
  assert_eq!(ended["code"], "not_stopped_at_limit");
}
