//! Secrets (07.01): adding a secret puts its value in the keychain and nothing else, the value
//! field is masked and emptied at once, a name taken in the environment keeps Save off, and a
//! value can be replaced and a secret archived. Values here are fake, and the keychain is the
//! harness's one in memory.

use std::cell::RefCell;

use orchester_core::{secret::Secret, store::SecretStore};
use orchester_store::ProjectDb;

use crate::harness::{Desktop, expected};

const NAME: &str = "STAGING_API_KEY";
const VALUE: &str = "fake-value-Q7x2";
const PRESSES: usize = 20;

fn secrets_screen() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-secrets");
  desktop.click("nav-secrets");
  desktop.wait_for("empty-add-secret");
  desktop
}

fn open_add_dialog(desktop: &mut Desktop, button: &str) {
  desktop.click(button);
  desktop.wait_for("secret-dialog");
}

/// Types `text` into the field `id`, as a user does.
fn type_into(desktop: &mut Desktop, id: &str, text: &str) {
  desktop.tree.get_element_by_id_mut(id).expect(id).focus();
  desktop.pass();
  desktop.type_text(text);
}

fn stored(desktop: &Desktop) -> Vec<Secret> {
  ProjectDb::open(desktop.repo())
    .expect("the project database")
    .secrets()
    .expect("the secrets")
}

/// Whether any text on screen holds `needle`.
fn shown(desktop: &mut Desktop, needle: &str) -> bool {
  !holders(desktop, needle).is_empty()
}

/// The ids of the elements whose text holds `needle`; an element without an id is listed by its
/// tag.
fn holders(desktop: &mut Desktop, needle: &str) -> Vec<String> {
  let found = RefCell::new(Vec::new());
  desktop.tree.find_element(|element| {
    if element.text_content().is_some_and(|text| text.contains(needle)) {
      let name = element.id().unwrap_or_else(|| element.tag_name());
      found.borrow_mut().push(name.to_owned());
    }
    false
  });
  found.into_inner()
}

fn add_secret(desktop: &mut Desktop, value: &str) {
  open_add_dialog(desktop, "empty-add-secret");
  type_into(desktop, "secret-name", NAME);
  type_into(desktop, "secret-value", value);
  desktop.click("secret-save");
  desktop.settle("the saved secret in the inspector", |tree| {
    tree.get_element_by_id("secret-dialog").is_none() && tree.get_element_by_id("secret-inspector").is_some()
  });
}

#[test]
fn secret_value_is_masked_and_leaves_the_field_when_saved() {
  let mut desktop = secrets_screen();
  open_add_dialog(&mut desktop, "empty-add-secret");
  type_into(&mut desktop, "secret-name", NAME);
  type_into(&mut desktop, "secret-value", VALUE);
  let input = desktop
    .tree
    .get_element_by_id_mut("secret-value")
    .and_then(|element| element.as_text_input())
    .expect("the value field");
  assert!(input.is_masked());
  assert_eq!(input.value(), VALUE);
  assert_eq!(
    holders(&mut desktop, VALUE),
    ["secret-value"],
    "only the masked field holds the value; inspectors and the MCP tree see its mask"
  );

  desktop.click("secret-save");
  assert_eq!(
    desktop.value("secret-value"),
    "",
    "the field is emptied as the save starts"
  );
  desktop.settle("the saved secret", |tree| {
    tree.get_element_by_id("secret-dialog").is_none()
  });

  assert_eq!(desktop.vault.values(), [VALUE]);
  let secrets = stored(&desktop);
  assert_eq!(secrets.len(), 1);
  assert_eq!(secrets[0].name().as_str(), NAME);
  desktop.wait_for("secret-inspector");
  assert!(shown(&mut desktop, NAME));
  assert!(!shown(&mut desktop, VALUE), "the inspector shows a mask only");
}

#[test]
fn secret_name_taken_in_the_environment_keeps_save_off_and_links_the_secret() {
  let mut desktop = secrets_screen();
  add_secret(&mut desktop, VALUE);
  open_add_dialog(&mut desktop, "add-secret");
  type_into(&mut desktop, "secret-name", NAME);
  type_into(&mut desktop, "secret-value", "fake-other-value");
  desktop.wait_for("secret-name-note");

  desktop.click("secret-save");
  desktop.pass();
  assert!(desktop.tree.get_element_by_id("secret-dialog").is_some());
  assert_eq!(desktop.vault.values(), [VALUE], "nothing else reached the keychain");

  desktop.click("secret-open-existing");
  assert!(desktop.tree.get_element_by_id("secret-dialog").is_none());
  let id = stored(&desktop)[0].id();
  desktop.wait_for(&format!("secret-{id}"));
}

#[test]
fn secret_replace_value_rotates_it() {
  let mut desktop = secrets_screen();
  add_secret(&mut desktop, VALUE);
  desktop.click("replace-secret-value");
  desktop.wait_for("secret-dialog");
  type_into(&mut desktop, "secret-value", "fake-rotated-value");
  desktop.click("secret-save");
  desktop.settle("the replaced value", |tree| {
    tree.get_element_by_id("secret-dialog").is_none()
  });

  assert_eq!(desktop.vault.values(), ["fake-rotated-value"]);
  let secrets = stored(&desktop);
  assert!(secrets[0].rotated_at().is_some());
}

#[test]
fn secret_archive_asks_first_and_removes_the_value() {
  let mut desktop = secrets_screen();
  add_secret(&mut desktop, VALUE);
  desktop.click("archive-secret");
  desktop.wait_for("archive-secret-dialog");
  desktop.click("archive-secret-cancel");
  assert!(desktop.tree.get_element_by_id("archive-secret-dialog").is_none());
  assert_eq!(desktop.vault.values(), [VALUE], "cancelled: nothing changed");

  desktop.click("archive-secret");
  desktop.wait_for("archive-secret-confirm");
  desktop.click("archive-secret-confirm");
  desktop.wait_for("empty-add-secret");
  assert!(desktop.vault.values().is_empty());
  assert!(stored(&desktop).is_empty());
}

#[test]
fn secret_dialog_traps_tab() {
  let mut desktop = secrets_screen();
  open_add_dialog(&mut desktop, "empty-add-secret");
  // Save stays off without a name and a value and is skipped; selects have no id of their own
  // and show up as unnamed stops.
  for stop in desktop.stops(PRESSES, false) {
    let stop = stop.unwrap_or_default();
    assert!(
      stop.is_empty()
        || ["secret-dialog-close", "secret-name", "secret-value", "secret-cancel"].contains(&stop.as_str()),
      "Tab left the dialog for {stop}"
    );
  }
  desktop.tab_to("secret-cancel", PRESSES);
  desktop.tab_to("secret-dialog-close", PRESSES);
  assert_eq!(
    desktop.stops(2, false),
    expected(&[Some("secret-name"), None]),
    "after the close control come the name, then the type select"
  );
}
