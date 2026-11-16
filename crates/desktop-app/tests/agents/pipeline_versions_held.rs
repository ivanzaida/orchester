//! The *Versions* button while its list is open (PV2-17, from PV2-11's review): it is drawn held, the menu-open fill
//! and border, which the window's tree says as `menu_open` (`ui::button`, the same branch that sets them), and plain
//! again once the list closes. `cargo test -p orchester-app --test agents pipeline_versions_held`.

use super::{
  pipeline_versions::{BUTTON, open_list, two_versions},
  team_hub_profile_fixes::Client,
};

/// The line of the window's tree that draws `id`.
fn line_of(tree: &str, id: &str) -> String {
  let at = tree.find(&format!("#{id} ")).expect("the element is in the tree");
  let end = tree[at..].find("\n").map_or(tree.len(), |end| at + end);
  tree[at..end].to_owned()
}

#[test]
fn pipeline_versions_held_the_button_is_held_while_its_list_is_open() {
  let (mut agents, _) = two_versions();
  let client = Client::new(&mut agents);
  let closed = line_of(&client.tree(&mut agents), BUTTON);
  assert!(!closed.contains("menu_open"), "plain while closed: {closed}");

  open_list(&mut agents);
  let open = line_of(&client.tree(&mut agents), BUTTON);
  assert!(open.contains("{menu_open=true}"), "held while open: {open}");

  agents.click(BUTTON);
  agents.until("the list closes", |s| !s.exists("pipeline-hub-versions"));
  let again = line_of(&client.tree(&mut agents), BUTTON);
  assert!(!again.contains("menu_open"), "plain once closed: {again}");
}
