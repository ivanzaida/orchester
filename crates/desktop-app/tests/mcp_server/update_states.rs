//! ORC-583: in a debug build, `set_update_state` shows any state `update_state` reports in place of this copy's real
//! one, taking what `update_state` answers, so the two mirror each other, and `real: true` shows the real state again.
//! Nothing of it reaches the application database. A release build does not serve it, nor `hold_reads`.
//!
//! `cargo test -p orchester-app --test mcp_server update_states_`

use orchester_app::notifications::update_ready::TOLD;
use serde_json::{Value, json};

use super::{Served, tool_value};

const RUNNING: &str = env!("CARGO_PKG_VERSION");

fn newest() -> Value {
  json!({
    "version": "0.4.0",
    "download": "https://api.orchester.cloud/v1/releases/0.4.0/files/Orchester-0.4.0-setup.exe",
  })
}

/// A ready update's answer in `state`.
fn ready(state: &str, critical: bool) -> Value {
  json!({ "state": state, "version": "0.4.0", "notes": "- Runs start faster.", "critical": critical })
}

/// `answer` with `fields` added.
fn with(mut answer: Value, fields: &Value) -> Value {
  for (name, value) in fields.as_object().expect("fields") {
    answer[name] = value.clone();
  }
  answer
}

/// Every state `update_state` reports, each kind and each of its reasons and steps, as it answers them.
fn every_state() -> Vec<Value> {
  let download = json!({ "version": "0.4.0", "received": 38_400_000, "total": 91_000_000, "percent": 42 });
  let mut states = vec![
    json!({ "state": "up_to_date", "checked_at": 1_790_000_000_000_i64 }),
    json!({ "state": "checking" }),
    with(json!({ "state": "downloading" }), &download),
    with(json!({ "state": "paused" }), &download),
    ready("ready", false),
    ready("ready", true),
    with(
      ready("waiting_for_runs", false),
      &json!({ "runs": ["ORC-110", "ORC-346"] }),
    ),
    with(
      ready("installing", false),
      &json!({ "step": "waiting_for_runs", "runs": ["ORC-110"] }),
    ),
    json!({ "state": "install_failed", "version": "0.4.0", "reason": "the installer ended before the app closed, with exit code 2", "exit_code": 2, "log": "C:\\Orchester\\updates\\install.log" }),
    json!({ "state": "install_failed", "version": "0.4.0", "reason": "cannot start the installer", "exit_code": null, "log": null }),
    json!({ "state": "cloud_unreachable", "last_checked": 1_790_000_000_000_i64 }),
    json!({ "state": "cloud_unreachable", "last_checked": null }),
    json!({ "state": "not_on_this_platform", "newest": null }),
    json!({ "state": "not_on_this_platform", "newest": newest() }),
  ];
  for step in ["stopping_runs", "backing_up", "handing_over"] {
    states.push(with(ready("installing", true), &json!({ "step": step })));
  }
  for reason in [
    "no_installer",
    "wrong_size",
    "wrong_digest",
    "unknown_key",
    "bad_signature",
    "not_saved",
  ] {
    states.push(json!({ "state": "failed", "version": "0.4.0", "reason": reason }));
  }
  for reason in [
    "development",
    "qa",
    "script_install",
    "portable",
    "program_folder_unknown",
  ] {
    states.push(json!({ "state": "not_this_copy", "reason": reason, "newest": null }));
    states.push(json!({ "state": "not_this_copy", "reason": reason, "newest": newest() }));
  }
  states
}

#[test]
fn update_states_set_update_state_takes_every_state_update_state_answers() {
  let mut served = Served::start(true);
  let session = served.session();
  let updated = json!({ "version": "0.4.0", "notes": "Runs start faster." });
  for (index, state) in every_state().into_iter().enumerate() {
    // Every other one with the update this start finished: the status bar's *Updated to* (S6).
    let finished = if index % 2 == 0 { updated.clone() } else { Value::Null };
    let answer = with(state, &json!({ "running": RUNNING, "updated": finished }));
    let (set, error) = tool_value(&served.call(&session, "set_update_state", &answer));
    assert!(!error, "{answer}: {set}");
    assert_eq!(set, with(answer.clone(), &json!({ "set_for_testing": true })));
    let (read, _) = tool_value(&served.call(&session, "update_state", &json!({})));
    assert_eq!(read, answer, "update_state answers what was set");
  }
}

#[test]
fn update_states_real_true_shows_the_real_state_again_and_nothing_is_stored() {
  let mut served = Served::start(true);
  let session = served.session();
  let services = served.launch.services().expect("a session").clone();
  let set = with(ready("ready", false), &json!({ "updated": null }));
  let (_, error) = tool_value(&served.call(&session, "set_update_state", &set));
  assert!(!error);
  assert!(services.updates.is_set_for_testing());
  served.tree.pass_headless(&mut served.app);
  assert_eq!(
    services.workspace.ui_state(TOLD).expect("the window state reads"),
    None,
    "the version told of a state set for testing stays in memory"
  );
  assert_eq!(
    services.updates.last_checked().expect("reads"),
    None,
    "no check ran for it"
  );

  let (real, error) = tool_value(&served.call(&session, "set_update_state", &json!({ "real": true })));
  assert!(!error, "{real}");
  assert_eq!(
    real,
    json!({
      "state": "not_this_copy",
      "reason": "development",
      "newest": null,
      "running": RUNNING,
      "updated": null,
      "set_for_testing": false,
    })
  );
}

#[test]
fn update_states_a_state_it_does_not_know_is_refused() {
  let mut served = Served::start(true);
  let session = served.session();
  let refusals = [
    (json!({ "state": "almost_ready" }), "invalid_param"),
    (json!({ "state": "ready" }), "missing_param"),
    (json!({ "state": "ready", "version": "four" }), "invalid_param"),
    (
      json!({ "state": "failed", "version": "0.4.0", "reason": "lost" }),
      "invalid_param",
    ),
    (
      json!({ "state": "waiting_for_runs", "version": "0.4.0" }),
      "missing_param",
    ),
    (json!({ "real": "yes" }), "not_a_bool"),
  ];
  for (args, code) in refusals {
    let (refusal, error) = tool_value(&served.call(&session, "set_update_state", &args));
    assert!(error, "{args} is refused");
    assert_eq!(refusal["code"], code, "{args}: {refusal}");
  }
  let (state, _) = tool_value(&served.call(&session, "update_state", &json!({})));
  assert_eq!(state["state"], "not_this_copy", "a refusal changes nothing");
}

/// The tools only debug builds serve are not there in a release build's server.
#[test]
fn update_states_a_release_build_serves_neither_set_update_state_nor_hold_reads() {
  let mut served = Served::start_as_release();
  let session = served.session();
  let names = served.endpoint().tool_names(&session);
  assert!(names.iter().any(|name| name == "update_state"), "{names:?}");
  for tool in ["set_update_state", "hold_reads"] {
    assert!(
      !names.iter().any(|name| name == tool),
      "{tool} is not listed: {names:?}"
    );
  }
  let refused = served.call(&session, "set_update_state", &ready("ready", false));
  assert!(
    refused.text.contains("unknown tool: set_update_state"),
    "{}",
    refused.text
  );
}
