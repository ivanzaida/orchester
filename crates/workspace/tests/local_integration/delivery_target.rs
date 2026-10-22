//! The Local integration page and the Summary say a run's work was merged without Orchester by one rule: the run's
//! delivery target, its base branch, holds the work. Another checked-out branch that holds it is not where it was
//! merged, and once the target holds it both name the target, whichever branch is checked out.

use orchester_core::store::RunStore;
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  delivery::{self, Delivery},
  local_integration::{IntegrationView, fingerprint, read},
};

use super::{
  privacy::Vault,
  support::{Fixture, git},
};

/// What the Summary reads of the run's work.
fn summary(fixture: &Fixture) -> Delivery {
  let run = ProjectDb::open(fixture.root())
    .expect("db")
    .run(fixture.run)
    .expect("read")
    .expect("run");
  delivery::of_run(&Programs::processes(), fixture.root(), &run)
}

fn page(fixture: &Fixture) -> IntegrationView {
  read(fixture.root(), fixture.run, None, &Vault(None)).expect("the page's view")
}

#[test]
fn delivery_target_another_checked_out_branch_holding_the_work_is_not_where_it_was_merged() {
  let fixture = Fixture::new(false);
  let root = fixture.root();
  git(root, &["checkout", "-b", "feature"]);
  git(root, &["merge", "--no-ff", "--no-edit", "task"]);
  let view = page(&fixture);
  assert_eq!(summary(&fixture), Delivery::Waiting, "the Summary still shows Ready");
  assert_eq!(
    view.delivery,
    summary(&fixture),
    "the page decides by the Summary's rule"
  );
  assert_eq!(
    view.merged_outside(),
    None,
    "feature holds the work, but main is where it goes"
  );
  assert!(!view.found_outside());
  let polled = fingerprint(root, fixture.run).expect("the poll");
  // main takes the work while feature stays checked out.
  git(root, &["branch", "-f", "main", "feature"]);
  assert_ne!(
    fingerprint(root, fixture.run).expect("the poll"),
    polled,
    "the poll sees the delivery target move"
  );
  let view = page(&fixture);
  assert_eq!(summary(&fixture), Delivery::MergedOutside("main".into()));
  assert_eq!(view.delivery, summary(&fixture));
  assert_eq!(view.merged_outside(), Some("main"), "both name the delivery target");
  assert!(view.found_outside());
  assert!(
    !view.outside_merge_provable(),
    "a merge into main can be recorded only with main checked out"
  );
}

#[test]
fn delivery_target_checked_out_holding_the_work_reads_merged_into_it() {
  let fixture = Fixture::new(false);
  git(fixture.root(), &["merge", "--ff-only", "task"]);
  let view = page(&fixture);
  assert_eq!(view.delivery, summary(&fixture));
  assert_eq!(view.merged_outside(), Some("main"));
  assert!(view.outside_merge_provable(), "the checkout's HEAD proves it");
}
