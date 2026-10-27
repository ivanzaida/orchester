//! Layout bounds for the redesigned Environment field and helper row, independent of screenshots.
use lurq::app::events::MouseButton;
use orchester_core::store::SettingsStore;

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
    // The editor runs the row's width inside its 14 sides (`Variables` `qGfsz` in `02 · Runs`).
    assert_eq!((field.width, field.height), (692.0, 132.0));
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
      "action aligns at the right of the helper, as wide as the field"
    );
    assert_eq!(action.height, 22.0);
  }
}

#[test]
fn project_settings_environment_pointer_focus_covers_132_body_and_insets_then_blur_saves() {
  const INPUT: &str = "settings-environment-variables";
  for (region, horizontal, vertical) in [
    ("first line", 0.1, 0.2),
    ("empty body", 0.5, 0.8),
    ("right edge", 0.99, 0.5),
    ("left inset", 0.01, 0.5),
    ("top inset", 0.5, 0.03),
    ("bottom inset", 0.5, 0.97),
  ] {
    let mut page = Settings::new(false);
    page.tab("environment", INPUT);
    let bounds = page
      .window
      .tree
      .get_element_by_id_mut("settings-environment-variables-box")
      .and_then(|e| e.bounds())
      .expect("Environment field bounds");
    assert_eq!(bounds.height, 132.0);
    let (x, y) = (
      bounds.x + bounds.width * horizontal,
      bounds.y + bounds.height * vertical,
    );
    page.window.tree.mouse_move(x, y);
    page.window.tree.mouse_down(x, y, MouseButton::Left);
    page.window.tree.mouse_up(x, y, MouseButton::Left);
    page.window.pass();
    assert_eq!(
      page.window.tree.focused_element().and_then(|e| e.id()),
      Some(INPUT),
      "{region} focuses the actual Environment input"
    );
    for key in ["C", "I", "=", "1"] {
      page
        .window
        .tree
        .key_down(key.to_owned(), String::new(), false, false, false);
      page.window.pass();
    }
    assert_eq!(page.value(INPUT), "CI=1", "typing after {region}");
    page
      .window
      .tree
      .get_element_by_id_mut("settings-open-secrets")
      .expect("helper action")
      .focus();
    page.window.pass();
    page.until("pointer edit saved on blur", |s| {
      s.db().project_environment().expect("Environment readback").get("CI") == Some("1")
    });
    page.saved();
  }
}
