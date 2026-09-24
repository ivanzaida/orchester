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
fn project_setup_tab_moves_between_back_name_and_folder() {
  let mut desktop = setup();
  // Back in the setup rail, then the form; Create stays disabled without a folder and is
  // skipped. Without a project the status bar has nothing to open.
  assert_eq!(
    desktop.stops(4, false),
    expected(&[
      Some("setup-back"),
      Some(NAME),
      Some("repository-path"),
      Some("setup-back")
    ])
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
