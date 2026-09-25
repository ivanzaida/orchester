use crate::harness::{Desktop, expected};

const NAME: &str = "project-name";

fn setup() -> Desktop {
  let mut desktop = Desktop::first_launch();
  desktop.wait_for("create-first-project");
  desktop.click("create-first-project");
  desktop.wait_for(NAME);
  desktop
}

#[test]
fn project_setup_tab_moves_between_name_and_folder() {
  let mut desktop = setup();
  // Only the form: Back in the setup rail is pointer-only, and Create stays disabled without
  // a folder and is skipped.
  assert_eq!(
    desktop.stops(3, false),
    expected(&[Some(NAME), Some("repository-path"), Some(NAME)])
  );
  assert_eq!(desktop.stops(2, true), expected(&[Some("repository-path"), Some(NAME)]));
}

#[test]
fn project_setup_enter_does_not_create_without_a_folder() {
  let mut desktop = setup();
  desktop.tab_to(NAME, 3);
  desktop.type_text("Kontur");
  desktop.enter();
  desktop.settle("the setup form", |tree| tree.get_element_by_id(NAME).is_some());
  assert_eq!(desktop.value(NAME), "Kontur");
}
