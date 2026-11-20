//! The window's input for history and tabs, acting on the active tab. The mouse's side buttons go back
//! and forward on release, as a browser does (the shell reports them as `Other(4)` and `Other(5)`), and so
//! does a trackpad swipe between pages that no scrolling list under the pointer took. The
//! key listener is the last node of the tree: keyboard events visit nodes from the root down in order,
//! so every control has seen a key before it and can claim it with `prevent_default`. The window's title
//! bar is not among them: lurq hands keys to the chrome's overlay, where it lies, after the page and this
//! listener.

use lurq::{
  app::events::{HistoryDirection, HistorySwipeEvent, KeyboardEvent, MouseButton, MouseEvent},
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::{
  Navigation,
  shortcuts::{Keys, Shortcut, shortcut},
};
use crate::routes::{ProjectPage, ProjectSettingsTab, Route};

const BACK_BUTTON: MouseButton = MouseButton::Other(4);
const FORWARD_BUTTON: MouseButton = MouseButton::Other(5);

/// `content` filling the window, with the history and tab input around it while `active`; none while the
/// closing panel holds the window.
pub fn window_input(navigation: &Navigation, content: Element, active: bool) -> Column {
  let (mouse, swipe, keys) = (navigation.clone(), navigation.clone(), navigation.clone());
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .on_mouse_up(move |event: MouseEvent| {
      if !active || event.default_prevented() {
        return;
      }
      match event.button {
        BACK_BUTTON => mouse.back(),
        FORWARD_BUTTON => mouse.forward(),
        _ => return,
      }
      event.prevent_default();
    })
    .on_history_swipe(move |event: HistorySwipeEvent| {
      if !active || event.default_prevented() {
        return;
      }
      match event.direction {
        HistoryDirection::Back => swipe.back(),
        HistoryDirection::Forward => swipe.forward(),
      }
      event.stop_propagation();
    })
    .child(content)
    .child(Column::new().on_key_down(move |event: KeyboardEvent| {
      if !active || event.default_prevented() {
        return;
      }
      let Some(shortcut) = shortcut(Keys::of(&event)) else {
        return;
      };
      event.prevent_default();
      run(&keys, shortcut);
    }))
}

fn run(navigation: &Navigation, shortcut: Shortcut) {
  match shortcut {
    Shortcut::Back => navigation.back(),
    Shortcut::Forward => navigation.forward(),
    Shortcut::NewTab => navigation.open_tab(),
    Shortcut::CloseTab => navigation.close_active(),
    Shortcut::NextTab => navigation.cycle(1),
    Shortcut::PreviousTab => navigation.cycle(-1),
    Shortcut::JumpTo => navigation.jump().update(|open| *open = !*open),
    // A tab that has been in no project has no project settings to open.
    Shortcut::ProjectSettings => {
      if let Some(project) = navigation.tabs_now().active().sidebar_project() {
        navigation.go(Route::Project(
          project,
          ProjectPage::Settings(ProjectSettingsTab::default()),
        ));
      }
    }
  }
}
