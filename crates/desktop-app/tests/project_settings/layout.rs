//! Project settings and Settings share one page (`ui::settings_page`): on every tab of both, at 1440 × 900 and 960
//! wide, the title, the tabs and a tab's form start at the same place with the same width.
use orchester_app::routes::{AccountsView, OrchesterView, ProjectPage, ProjectSettingsTab, Route};

use super::harness::Settings;
use crate::support::project_id;

/// Where the page puts its title, its tabs and, on a tab with one, its form: x, y and width.
#[derive(Debug)]
struct Placed {
  title: (f32, f32),
  tabs: (f32, f32, f32),
  form: Option<(f32, f32)>,
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

/// The page `route` shows, once its tabs `tabs` (and its form, if `form`) are laid out.
fn placed(page: &mut Settings, route: Route, tabs: &str, form: bool) -> Placed {
  page.window.navigation().go(route);
  page.until("the page", |s| s.exists(tabs) && (!form || s.exists("settings-form")));
  let title = bounds(page, "settings-title");
  Placed {
    title: (title.0, title.1),
    tabs: bounds(page, tabs),
    form: form.then(|| {
      let form = bounds(page, "settings-form");
      (form.0, form.2)
    }),
  }
}

fn same(a: f32, b: f32) -> bool {
  (a - b).abs() < 0.5
}

#[test]
fn project_settings_lines_up_with_settings_on_every_tab() {
  let mut page = Settings::new(false);
  for (width, height) in [(1440, 900), (960, 900)] {
    page.window.tree.resize(width, height);
    page.window.pass();
    let settings = [
      (Route::Settings, true),
      (Route::Accounts(AccountsView::default()), false),
      (Route::Orchester(OrchesterView::default()), false),
    ];
    let mut pages: Vec<(String, Placed)> = settings
      .into_iter()
      .map(|(route, form)| (format!("{route:?}"), placed(&mut page, route, "settings-tabs", form)))
      .collect();
    pages.extend(ProjectSettingsTab::ALL.map(|tab| {
      let route = Route::Project(project_id(), ProjectPage::Settings(tab));
      (
        format!("{tab:?}"),
        placed(&mut page, route, "project-settings-tabs", true),
      )
    }));
    let (_, first) = &pages[0];
    let form = first.form.expect("Automation has a form");
    for (name, placed) in &pages {
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
      if let Some(placed) = placed.form {
        assert!(same(placed.0, form.0) && same(placed.1, form.1), "form, {at}");
        assert!(same(placed.0, first.tabs.0), "the form under the tabs' start, {at}");
      }
    }
  }
}
