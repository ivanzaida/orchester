//! `merge_run` of a team task's run (P7b) against the fake server's delivery commands, in a repository on `main` whose
//! module branch is one commit ahead of it: the run's module is claimed and its delivery requested, which the server
//! keeps waiting for an Admin's authorization (S12) unless an authorization is cited or the automatic merge is on; once
//! an Admin authorized the waiting request, the next call takes the merge; a delivery whose merge waits is carried on,
//! never requested again; a checkout on the module's own branch has no delivery; and a run with no team record (a local
//! task's, also in a team project) keeps `merge_run`'s own path.

use orchester_core::id::OperationId;
use orchester_exec::local_git::IntegrationLease;
use serde_json::{Value, json};

use crate::{
  Checkout, git,
  team_runs::{Started, started},
  team_server::{MEMBER, MODULE_BRANCH},
  tools::{canceled_run, server},
};

/// A team task's run in its module, canceled: its work no longer runs here.
fn ended() -> Started {
  let team = started();
  team.settle(|run, now| run.cancel(now).expect("canceled"));
  team
}

fn merge() -> Value {
  json!({"project": "Kontur", "run": "1"})
}

/// The merge commits on `main`.
fn merges(checkout: &Checkout) -> Vec<String> {
  git(checkout.root(), &["rev-list", "--merges", "main"])
    .lines()
    .map(str::to_owned)
    .collect()
}

/// `merge_run`'s answer for a delivered module: the module branch merged once into `main`, as the server keeps it.
fn assert_delivered(checkout: &Checkout, delivered: &Value) -> String {
  let tip = git(checkout.root(), &["rev-parse", "main"]);
  assert_eq!(delivered["delivery"]["state"], "delivered", "{delivered}");
  assert_eq!(delivered["delivery"]["branch"], MODULE_BRANCH);
  assert_eq!(delivered["delivery"]["into"], "main");
  assert_eq!(delivered["delivery"]["merge_commit"], tip.as_str());
  assert_eq!(
    delivered["delivery"]["source_commit"],
    git(checkout.root(), &["rev-parse", MODULE_BRANCH]).as_str()
  );
  assert_eq!(delivered["message"], "appMcp.teamDelivered");
  assert_eq!(delivered["team"], true);
  assert_eq!(merges(checkout).len(), 1, "merged once");
  tip
}

#[test]
fn team_mcp_merge_run_requests_the_module_delivery() {
  let Started { checkout, server, .. } = ended();

  // Without an Admin's authorization or the automatic merge the server keeps the request waiting for an Admin.
  assert_eq!(checkout.refused("merge_run", &merge()), "delivery_awaiting_admin");
  {
    let state = server.state();
    assert_eq!(state.delivery_requests, 1, "the delivery was requested");
    assert!(
      state.module_claim.as_ref().is_some_and(|claim| claim.token == MEMBER),
      "under the module's claim, taken in this session"
    );
    assert_eq!(state.deliveries.len(), 1, "the request is kept");
    assert_eq!(state.deliveries[0].waits, Some("admin"));
    assert_eq!(state.deliveries[0].authorization, None, "the tool authorized nothing");
  }
  assert!(merges(&checkout).is_empty(), "nothing merged");

  // The workspace's automatic merge allows the waiting request asked again: the module branch lands on main and is
  // reported.
  server.state().automatic_merge = true;
  let delivered = checkout.call("merge_run", &merge()).expect("delivered");
  let tip = assert_delivered(&checkout, &delivered);
  assert_eq!(delivered["delivery"]["authorization"], Value::Null);
  let state = server.state();
  assert_eq!(state.deliveries.len(), 1);
  assert_eq!(state.deliveries[0].state, "delivered");
  assert_eq!(state.deliveries[0].merge_commit.as_deref(), Some(tip.as_str()));
}

#[test]
fn team_mcp_merge_run_carries_on_a_waiting_delivery() {
  let Started { checkout, server, .. } = ended();
  server.state().automatic_merge = true;

  // Another merge holds the checkout: the server took the request, and the merge waits.
  let lease = IntegrationLease::acquire(checkout.root()).expect("the checkout held");
  assert_eq!(checkout.refused("merge_run", &merge()), "team_delivery_merge_waiting");
  assert!(
    merges(&checkout).is_empty(),
    "nothing merged while the checkout is held"
  );
  drop(lease);

  // Called again, the same delivery goes on: the server would refuse a second request of the claim.
  let delivered = checkout.call("merge_run", &merge()).expect("delivered");
  assert_delivered(&checkout, &delivered);
  let state = server.state();
  assert_eq!(
    (state.delivery_requests, state.delivery_reports),
    (1, 1),
    "requested and reported once"
  );
}

#[test]
fn team_mcp_merge_run_cites_an_admins_authorization() {
  let Started { checkout, server, .. } = ended();
  assert_eq!(checkout.refused("merge_run", &merge()), "delivery_awaiting_admin");

  // An Admin authorizes the waiting request of the module's claim on their own computer, under their own operation.
  let authorization = OperationId::generate().to_string();
  {
    let mut state = server.state();
    let waiting = &mut state.deliveries[0];
    assert_eq!(waiting.waits, Some("admin"));
    waiting.authorization = Some(authorization.clone());
    waiting.waits = Some("holder");
  }

  // Called again with it, the new request cites it, and the module lands on main.
  let mut cited = merge();
  cited["authorization"] = json!(authorization);
  let delivered = checkout.call("merge_run", &cited).expect("delivered");
  assert_delivered(&checkout, &delivered);
  assert_eq!(delivered["delivery"]["authorization"], authorization.as_str());
  let state = server.state();
  assert_eq!(
    state.delivery_requests, 2,
    "waited once, then requested on the authorization"
  );
  assert_eq!(state.deliveries.len(), 1);
  assert_eq!(state.deliveries[0].state, "delivered");
}

#[test]
fn team_mcp_merge_run_of_a_local_tasks_run_in_a_team_project_merges_locally() {
  let (server, _seen) = server();
  // A run kept from before the project was linked: no team record, so it is a local run whatever the link says.
  let checkout = Checkout::prepared(Some(&server), &["Say hello"], canceled_run);

  // The local merge's own checks answer: a canceled run is not ready to merge.
  assert_eq!(checkout.refused("merge_run", &merge()), "run_not_ready");
  let mut cited = merge();
  cited["authorization"] = json!(OperationId::generate().to_string());
  assert_eq!(checkout.refused("merge_run", &cited), "invalid_arguments");
  let state = server.state();
  assert_eq!(state.delivery_requests, 0, "nothing requested");
  assert!(state.module_claim.is_none(), "nothing claimed");
  drop(state);
  assert!(merges(&checkout).is_empty());
}

#[test]
fn team_mcp_merge_run_on_the_module_branch_is_refused() {
  let Started { checkout, server, .. } = ended();
  server.state().automatic_merge = true;
  git(checkout.root(), &["switch", MODULE_BRANCH]);

  // The module's branch cannot be delivered onto itself: nothing is claimed or requested.
  assert_eq!(checkout.refused("merge_run", &merge()), "module_branch_checked_out");
  let state = server.state();
  assert_eq!(state.delivery_requests, 0, "nothing requested");
  assert!(state.module_claim.is_none(), "nothing claimed");
}

#[test]
fn team_mcp_local_merge_run_keeps_its_own_path() {
  let checkout = Checkout::prepared(None, &["Sign in"], canceled_run);

  // The local merge's own checks answer: a canceled run is not ready to merge.
  assert_eq!(checkout.refused("merge_run", &merge()), "run_not_ready");
  // A delivery's authorization is a team project's only.
  let mut cited = merge();
  cited["authorization"] = json!(OperationId::generate().to_string());
  assert_eq!(checkout.refused("merge_run", &cited), "invalid_arguments");
  assert!(merges(&checkout).is_empty());
}
