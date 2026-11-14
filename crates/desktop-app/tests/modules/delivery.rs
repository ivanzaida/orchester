//! A local module's own branch (`M01 · Edit module · its own branch`, `M02 · Module · Summary · its branch and
//! Deliver`; OA-17, OA-20): the Summary of a module with a branch of its own starts with its delivery row and names
//! the branch, its acceptance and who delivers it among the properties, and a module without one shows none of it;
//! Edit module turns the branch on under *Delivery*, with the acceptance pipeline and *Deliver when accepted* only
//! while it is on, and the save keeps it.

use lurq::app::events::MouseButton;
use orchester_app::routes::{ModuleTab, ProjectPage};
use orchester_core::{module::delivery::OwnBranch, run::BranchName, task::organization::WorkflowStatus};

use super::harness::{Seeded, details, module_with};

#[test]
fn modules_summary_of_a_module_with_its_own_branch_starts_with_its_delivery() {
  let mut picked = None;
  let mut plain = None;
  let mut seeded = Seeded::new("tasks", |db| {
    let (module, _) = module_with(db, details("Checkout", (3, 4)), &[(WorkflowStatus::Todo, 1)]);
    let own = OwnBranch::new(BranchName::new("module/checkout").expect("a branch name"));
    db.set_module_own_branch(module, Some(&own)).expect("its own branch");
    picked = Some(module);
    plain = Some(module_with(db, details("Payments", (3, 4)), &[]).0);
  });
  let (module, plain) = (picked.expect("the module"), plain.expect("the other module"));
  seeded.open(ProjectPage::Module(module, ModuleTab::Summary), "module-summary");
  seeded.until("the delivery row", |seeded| seeded.shows("module-delivery"));
  assert_eq!(seeded.text("module-delivery-branch"), "module/checkout");
  assert_eq!(seeded.text("module-delivery-into"), "the checked-out branch");
  assert!(!seeded.shows("module-delivery-state"), "no state to say yet");
  assert!(
    seeded.text("module-delivery-detail").starts_with("No task merged yet"),
    "{}",
    seeded.text("module-delivery-detail")
  );
  assert!(!seeded.shows("module-deliver"), "nothing to deliver yet");
  assert_eq!(seeded.text("module-property-branch-value"), "module/checkout");
  assert_eq!(seeded.text("module-property-acceptance-value"), "None");
  assert_eq!(seeded.text("module-property-deliver-value"), "By you");

  seeded.open(ProjectPage::Module(plain, ModuleTab::Summary), "module-summary");
  assert_eq!(seeded.text("module-title"), "Payments");
  assert!(
    !seeded.shows("module-delivery") && !seeded.shows("module-property-branch"),
    "a module without a branch of its own shows no delivery"
  );
}

/// A press with the pointer at the middle of `id`, as a person presses a switch.
fn press(seeded: &mut Seeded, id: &str) {
  let bounds = seeded
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  let tree = &mut seeded.window.tree;
  tree.mouse_move(x, y);
  tree.mouse_down(x, y, MouseButton::Left);
  tree.mouse_up(x, y, MouseButton::Left);
  seeded.window.pass();
}

#[test]
fn modules_edit_module_gives_the_module_a_branch_of_its_own() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (3, 4)), &[]).0);
  });
  let module = picked.expect("the module");
  seeded.open(ProjectPage::EditModule(module), "module-form");
  assert!(seeded.shows("module-delivery"), "the Delivery group");
  assert!(
    !seeded.shows("module-acceptance-row") && !seeded.shows("module-deliver-when-accepted-row"),
    "its lower rows show only while the branch is on"
  );
  press(&mut seeded, "module-own-branch");
  seeded.until("the lower rows", |seeded| seeded.shows("module-acceptance-row"));
  assert!(seeded.shows("module-deliver-when-accepted-row"));
  assert_eq!(seeded.options("module-acceptance")[0], "None");

  seeded.click("module-editor-save");
  seeded.until("the module's page", |seeded| seeded.shows("module-summary"));
  let stored = seeded.module_named("Checkout").expect("stored");
  let own = stored.own_branch().expect("a branch of its own");
  assert_eq!(own.branch.as_str(), "module/checkout");
  assert_eq!((own.acceptance, own.deliver_when_accepted), (None, false));
  seeded.until("the delivery row", |seeded| seeded.shows("module-delivery"));
}
