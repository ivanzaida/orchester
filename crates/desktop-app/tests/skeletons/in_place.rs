//! Real content takes the bones' place in the same room, and a section that reads inside a shown page leaves the rest
//! of the page as it is.

use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};

use crate::harness::Held;

#[test]
fn skeletons_real_rows_replace_the_bones_without_changing_the_groups_height() {
  let mut held = Held::new();
  held.page(ProjectPage::Settings(ProjectSettingsTab::General));
  held.skeleton("settings-loading");
  let bones = held.bounds("settings-project-reading");
  held.release("settings-project");
  let read = held.bounds("settings-project");
  assert!(!held.exists("settings-loading"), "the bones are gone");
  assert!(
    (read.y - bones.y).abs() < 0.5,
    "the group stands where its bones stood: {} vs {}",
    read.y,
    bones.y
  );
  assert!(
    (read.height - bones.height).abs() < 0.5,
    "the group is as high as its bones: {} vs {}",
    read.height,
    bones.height
  );
}

#[test]
fn skeletons_a_value_still_read_keeps_the_rest_of_its_page() {
  let mut held = Held::new();
  held.go(Route::General);
  held.skeleton("settings-threshold-reading");
  assert!(held.exists("settings-appearance"), "the theme, read already, stays as it is");
  assert!(held.exists("settings-threshold-row"), "the threshold's row stands round its bone");
  held.release("settings-threshold-value");
  assert!(!held.exists("settings-threshold-reading"), "the value takes its bone's place");
}
