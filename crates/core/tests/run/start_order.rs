//! The start order the coordinator hands the executor crosses a process boundary as a value: it reads back as it was
//! written.

use orchester_core::{
  id::RunId,
  revision::Revision,
  run::{BranchName, StartOrder},
};

#[test]
fn a_start_order_round_trips_through_its_serialized_form() {
  let order = StartOrder::new(
    RunId::generate(),
    BranchName::new("orchester/orc-677-run-2").expect("branch"),
    Revision::new(&"a1".repeat(20)).expect("revision"),
    true,
  );

  let written = serde_json::to_string(&order).expect("write");
  let read: StartOrder = serde_json::from_str(&written).expect("read");

  assert_eq!(read, order);
  assert_eq!(read.branch().as_str(), "orchester/orc-677-run-2");
  assert!(read.borrows());
}

#[test]
fn a_start_order_with_a_branch_or_base_git_cannot_take_is_not_read() {
  let order = StartOrder::new(
    RunId::generate(),
    BranchName::new("orchester/orc-677-run-1").expect("branch"),
    Revision::new(&"b2".repeat(20)).expect("revision"),
    false,
  );
  let written = serde_json::to_value(&order).expect("write");

  let mut bad_branch = written.clone();
  bad_branch["branch"] = "orchester/..".into();
  assert!(serde_json::from_value::<StartOrder>(bad_branch).is_err());
  let mut bad_base = written;
  bad_base["base"] = "HEAD".into();
  assert!(serde_json::from_value::<StartOrder>(bad_base).is_err());
}
