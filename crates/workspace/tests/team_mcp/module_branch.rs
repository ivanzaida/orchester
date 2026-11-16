//! OA-33: a start makes a module's branch the checkout does not have yet, at the tip of the branch it has checked
//! out, without moving a branch that exists or leaving the branch the checkout is on; with no branch checked out
//! there is nothing to make it from, and the start is refused as before.

use std::fs;

use orchester_exec::programs::Programs;
use orchester_workspace::team_work::start_tip;

use crate::{Checkout, git, team_server::MODULE_BRANCH, tools::server};

#[test]
fn team_mcp_a_start_makes_the_module_branch_the_checkout_does_not_have() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let root = checkout.root();
  let programs = Programs::processes();
  let tip = || start_tip(programs.runner(), root, MODULE_BRANCH);

  // The checkout has it: its tip, as before.
  let had = git(root, &["rev-parse", MODULE_BRANCH]);
  assert_eq!(tip().expect("its tip").as_str(), had);

  // It does not: the branch is made at the tip of the branch checked out, and the checkout stays where it is.
  git(root, &["branch", "-D", MODULE_BRANCH]);
  let made = tip().expect("made");
  assert_eq!(made.as_str(), git(root, &["rev-parse", "main"]));
  assert_eq!(git(root, &["rev-parse", MODULE_BRANCH]), made.as_str());
  assert_eq!(git(root, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");

  // A branch that exists is never moved, however far the base went on.
  fs::write(root.join("later.txt"), "later\n").expect("a file");
  git(root, &["add", "later.txt"]);
  git(root, &["commit", "-m", "later"]);
  assert_eq!(tip().expect("its tip"), made, "still where it was made");

  // No branch is checked out: nothing to make it from.
  git(root, &["branch", "-D", MODULE_BRANCH]);
  git(root, &["switch", "--detach"]);
  assert!(tip().is_err(), "refused, and no branch was made");
  let branches = git(root, &["branch", "--list", MODULE_BRANCH]);
  assert!(branches.is_empty(), "{branches}");
}
