//! Layout bounds for the redesigned Environment field and helper row, independent of screenshots.
use super::harness::Settings;

#[test]
fn project_settings_environment_field_and_helper_keep_the_drawn_bounds() {
  let mut page = Settings::new(false);
  page.tab("environment", "settings-environment-variables");
  for text in [
    "",
    "BUILD_MODE=release\nRUST_LOG=info\nCI=1\nLANG=en_US.UTF-8\nCOLOR=always",
  ] {
    page.set("settings-environment-variables", text);
    let field = page
      .window
      .tree
      .get_element_by_id_mut("settings-environment-variables-box")
      .and_then(|e| e.bounds())
      .expect("field bounds");
    assert_eq!((field.width, field.height), (720.0, 132.0));
    let helper = page
      .window
      .tree
      .get_element_by_id_mut("settings-environment-helper")
      .and_then(|e| e.bounds())
      .expect("helper");
    let action = page
      .window
      .tree
      .get_element_by_id_mut("settings-open-secrets")
      .and_then(|e| e.bounds())
      .expect("Open Secrets");
    assert!((action.y - helper.y).abs() < 0.5, "action stays on helper row");
    assert!(
      (action.x + action.width - helper.x - helper.width).abs() < 0.5,
      "action aligns at the right of the same 720-wide helper"
    );
    assert_eq!(action.height, 22.0);
  }
}
