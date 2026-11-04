//! `merge_run` of a team task's run (P7b) against the fake server's delivery commands, in a repository on `main` whose
//! module branch is one commit ahead of it: the run's module is claimed and its delivery requested, which waits for an
//! Admin's authorization, cited by its operation, or the automatic merge; a delivery whose merge waits is carried on,
//! never requested again; a run outside a module, or a checkout on the module's own branch, has no delivery; and a
//! local project's `merge_run` keeps its own path.

use orchester_core::{id::OperationId, integration::base::RunBaseSource};
use orchester_exec::local_git::IntegrationLease;
use serde_json::{Value, json};

use crate::{
  Checkout, git,
  team_server::{Delivery, MEMBER, MODULE_BRANCH},
  tools::{canceled_run, run_from, server},
};

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
  let (server, _seen) = server();
  let module = server.module;
  let checkout = Checkout::prepared(Some(&server), &["Say hello"], |db| {
    run_from(db, (Some(MODULE_BRANCH), RunBaseSource::Module { module }));
  });

  // Without an Admin's authorization or the automatic merge the server refuses the request: it waits for one.
  assert_eq!(checkout.refused("merge_run", &merge()), "delivery_not_authorized");
  {
    let state = server.state();
    assert_eq!(state.delivery_requests, 1, "the delivery was requested");
    assert!(
      state.module_claim.as_ref().is_some_and(|claim| claim.token == MEMBER),
      "under the module's claim, taken in this session"
    );
    assert!(state.deliveries.is_empty(), "the tool authorized nothing");
  }
  assert!(merges(&checkout).is_empty(), "nothing merged");

  // The workspace's automatic merge authorizes the next request: the module branch lands on main and is reported.
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
  let (server, _seen) = server();
  let module = server.module;
  let checkout = Checkout::prepared(Some(&server), &["Say hello"], |db| {
    run_from(db, (Some(MODULE_BRANCH), RunBaseSource::Module { module }));
  });
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
  let (server, _seen) = server();
  let module = server.module;
  let checkout = Checkout::prepared(Some(&server), &["Say hello"], |db| {
    run_from(db, (Some(MODULE_BRANCH), RunBaseSource::Module { module }));
  });
  assert_eq!(checkout.refused("merge_run", &merge()), "delivery_not_authorized");

  // An Admin authorizes this delivery of the module's claim on their own computer, under their own operation.
  let authorization = OperationId::generate().to_string();
  {
    let mut state = server.state();
    let claim = state.module_claim.as_ref().expect("the module claimed").id;
    state.deliveries.push(Delivery {
      authorization: Some(authorization.clone()),
      claim,
      branch: MODULE_BRANCH.to_owned(),
      source_commit: git(checkout.root(), &["rev-parse", MODULE_BRANCH]),
      base_commit: git(checkout.root(), &["rev-parse", "main"]),
      state: "authorized".to_owned(),
      revision: 1,
      merge_commit: None,
    });
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
    "refused once, then requested on the authorization"
  );
  assert_eq!(state.deliveries.len(), 1);
  assert_eq!(state.deliveries[0].state, "delivered");
}

#[test]
fn team_mcp_merge_run_of_a_run_outside_a_module_is_refused() {
  let (server, _seen) = server();
  // A run kept from before the project was linked, started on the checked-out branch.
  let checkout = Checkout::prepared(Some(&server), &["Say hello"], canceled_run);

  assert_eq!(checkout.refused("merge_run", &merge()), "module_required");
  let state = server.state();
  assert_eq!(state.delivery_requests, 0, "nothing requested");
  assert!(state.module_claim.is_none(), "nothing claimed");
  drop(state);
  assert!(merges(&checkout).is_empty());
}

#[test]
fn team_mcp_merge_run_on_the_module_branch_is_refused() {
  let (server, _seen) = server();
  let module = server.module;
  let checkout = Checkout::prepared(Some(&server), &["Say hello"], |db| {
    run_from(db, (Some(MODULE_BRANCH), RunBaseSource::Module { module }));
  });
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
