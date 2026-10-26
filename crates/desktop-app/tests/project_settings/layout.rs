//! Project settings' page (`Project settings · redesign` `XGtkg`): on each of its six tabs, Connections and Secrets
//! included, at 1440 × 900 and 960 wide, the title, the tabs and the tab's column start at the same place.
use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};

use super::harness::Settings;
use crate::support::project_id;

/// Where the page puts its title, its tabs and its column: x, y and width.
#[derive(Debug)]
struct Placed {
  title: (f32, f32),
  tabs: (f32, f32, f32),
  column: (f32, f32),
}

fn bounds(page: &mut Settings, id: &str) -> (f32, f32, f32) {
  let bounds = page
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.x, bounds.y, bounds.width)
}

/// The tab `page_of` shows, once its column is laid out.
fn placed(page: &mut Settings, page_of: ProjectPage) -> Placed {
  page.window.navigation().go(Route::Project(project_id(), page_of));
  page.until("the page", |s| {
    s.exists("project-settings-tabs") && s.exists("settings-form")
  });
  let title = bounds(page, "settings-title");
  let column = bounds(page, "settings-form");
  Placed {
    title: (title.0, title.1),
    tabs: bounds(page, "project-settings-tabs"),
    column: (column.0, column.2),
  }
}

fn same(a: f32, b: f32) -> bool {
  (a - b).abs() < 0.5
}

#[test]
fn project_settings_tabs_line_up_on_every_tab() {
  let mut page = Settings::new(false);
  for (width, height) in [(1440, 900), (960, 900)] {
    page.window.tree.resize(width, height);
    page.window.pass();
    let mut pages: Vec<ProjectPage> = ProjectSettingsTab::ALL.map(ProjectPage::Settings).to_vec();
    pages.push(ProjectPage::Connections(Default::default()));
    pages.push(ProjectPage::Secrets(Default::default()));
    let placed: Vec<(String, Placed)> = pages
      .into_iter()
      .map(|shown| (format!("{shown:?}"), placed(&mut page, shown)))
      .collect();
    let (_, first) = &placed[0];
    for (name, placed) in &placed {
      let at = format!("{name} at {width} wide: {placed:?} against {first:?}");
      assert!(
        same(placed.title.0, first.title.0) && same(placed.title.1, first.title.1),
        "title, {at}"
      );
      assert!(
        same(placed.tabs.0, first.tabs.0) && same(placed.tabs.1, first.tabs.1) && same(placed.tabs.2, first.tabs.2),
        "tabs, {at}"
      );
      assert!(
        same(placed.title.0, placed.tabs.0),
        "the title over the tabs' start, {at}"
      );
      assert!(
        same(placed.column.0, first.column.0) && same(placed.column.1, first.column.1),
        "column, {at}"
      );
    }
  }
}
