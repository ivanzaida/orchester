use crate::harness::Desktop;

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
  desktop.tab();
  assert_eq!(desktop.focused_input(&[NAME]).as_deref(), Some(NAME));
  // The folder chooser; Create stays disabled without a folder and is skipped.
  desktop.tab();
  assert_eq!(desktop.focused_input(&[NAME]), None);
  desktop.tab();
  assert_eq!(desktop.focused_input(&[NAME]).as_deref(), Some(NAME));
  desktop.shift_tab();
  assert_eq!(desktop.focused_input(&[NAME]), None);
  desktop.shift_tab();
  assert_eq!(desktop.focused_input(&[NAME]).as_deref(), Some(NAME));
}

#[test]
fn project_setup_enter_does_not_create_without_a_folder() {
  let mut desktop = setup();
  desktop.tab();
  desktop.type_text("Kontur");
  desktop.enter();
  desktop.settle("the setup form", |tree| tree.get_element_by_id(NAME).is_some());
  assert_eq!(desktop.value(NAME), "Kontur");
}
