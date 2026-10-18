//! Places an Orchestrator's reply names are links (`ui::chat::references`): a key of one of the project's tasks and
//! an app route path of a place in a project, as text or as all a code span holds; an unknown key, a route outside a
//! project and a path inside longer code stay as written. Every in-app link opens by a browser's rules
//! (`support::link_press`); a link to the web never opens for synthetic input, whatever the button or keys.

use lurq::app::events::{MouseButton, with_synthetic_input};
use orchester_app::{
  navigation::press::LinkPress,
  routes::{OrchestratorChat, ProjectPage, Route, TaskView},
  ui::chat::links::{Followed, follow},
};

use super::{
  harness::{Orchestrator, Setup},
  support::link_press::opens_by_browser_rules,
};

#[test]
fn orchestrator_reply_links_task_keys_and_routes_and_opens_them_by_browser_rules() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let project = screen.project;
  let pipeline = screen.delivery();
  let key = screen.draft_on(pipeline, "Delivery log");
  screen.window.projects().feed(project).saved();
  let conversation = format!("/project/{project}/tasks/{key}/conversation");
  let changes = format!("/project/{project}/tasks/{key}/changes");
  let nowhere = format!("/project/{project}/nowhere");
  let longer = format!("/project/{project}/tasks");
  screen.go(OrchestratorChat::New);
  screen.send(&format!(
    "Start with {key}, then {conversation} and `{changes}`. Not KON-99, /settings or {nowhere}; not `open {longer}`."
  ));
  screen.until("the linked reply", |screen| {
    screen
      .window
      .tree
      .find_element(|element| element.text_content() == Some(&key))
      .is_some()
  });
  for text in [conversation.as_str(), key.as_str(), changes.as_str()] {
    assert!(
      screen
        .window
        .tree
        .find_element(|element| element.text_content() == Some(text))
        .is_some(),
      "{text} is a link"
    );
  }
  for text in ["KON-99", "/settings", nowhere.as_str(), longer.as_str()] {
    assert!(
      screen
        .window
        .tree
        .find_element(|element| element.text_content() == Some(text))
        .is_none(),
      "{text} stays text"
    );
  }

  let summary = Route::Project(
    project,
    ProjectPage::Task(key.parse().expect("a task key"), TaskView::Summary),
  );
  opens_by_browser_rules(&mut screen.window, &key, &summary);
  screen.window.navigation().back();
  screen.until("the chat again", |screen| {
    screen
      .window
      .tree
      .find_element(|element| element.text_content() == Some(&*conversation))
      .is_some()
  });
  let place = Route::parse(&conversation).expect("the Conversation route");
  opens_by_browser_rules(&mut screen.window, &conversation, &place);

  let tabs = screen.window.navigation().tabs_now();
  for button in [MouseButton::Left, MouseButton::Middle] {
    let press = LinkPress {
      button,
      shift: false,
      command: true,
    };
    let followed = with_synthetic_input(|| {
      follow(
        &screen.window.navigation(),
        "https://github.com/acme/kontur/pull/42",
        press,
      )
    });
    assert!(
      matches!(followed, Followed::NotOpened),
      "synthetic input never opens the browser: {followed:?}"
    );
  }
  assert_eq!(screen.window.navigation().tabs_now(), tabs, "nor a tab");
}
