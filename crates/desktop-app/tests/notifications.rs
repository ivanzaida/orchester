//! OS notifications in the app (ORC-56): only the installed app shows them unless a development build or a QA copy is
//! asked to, and every place a notification opens is a route of the app's router.

use std::ffi::OsString;

use orchester_app::{
  notifications::os::Delivery,
  routes::{OrchestratorChat, OverviewTab, ProjectPage, Route, TaskView},
};
use orchester_core::{
  id::{AgentAccountId, ChatId, ProjectId},
  project::TaskKey,
};
use orchester_workspace::{build::Build, links::TaskPage, notifications::Target};

#[test]
fn notifications_show_only_from_the_installed_app_unless_asked() {
  assert!(matches!(Delivery::choose(Build::Installed, None), Delivery::Shows(_)));
  assert_eq!(
    Delivery::choose(Build::Other, None),
    Delivery::Silent,
    "a QA copy stays silent"
  );
  assert_eq!(
    Delivery::choose(Build::Other, Some(OsString::from("0"))),
    Delivery::Silent
  );
  let Delivery::Shows(asked) = Delivery::choose(Build::Other, Some(OsString::from("1"))) else {
    panic!("asked to show them");
  };
  let Delivery::Shows(installed) = Delivery::choose(Build::Installed, None) else {
    panic!("the installed app shows them");
  };
  assert_ne!(
    asked, installed,
    "a development build never mixes with the installed app's"
  );
}

#[test]
fn notifications_open_routes_of_the_app() {
  let project = ProjectId::generate();
  let key: TaskKey = "ORC-18".parse().expect("a key");
  let chat = ChatId::generate();
  let task = |page| Target::Task {
    project,
    key: key.clone(),
    page,
    card: None,
  };
  let cases = [
    (
      task(TaskPage::Conversation),
      Route::Project(project, ProjectPage::Task(key.clone(), TaskView::Conversation)),
    ),
    (
      task(TaskPage::Summary),
      Route::Project(project, ProjectPage::Task(key.clone(), TaskView::Summary)),
    ),
    (
      task(TaskPage::LocalIntegration),
      Route::Project(project, ProjectPage::Task(key.clone(), TaskView::LocalIntegration)),
    ),
    (
      Target::Runner(project),
      Route::Project(project, ProjectPage::Overview(OverviewTab::default())),
    ),
    (
      Target::Orchestrator(project, chat),
      Route::Project(project, ProjectPage::Orchestrator(OrchestratorChat::Chat(chat))),
    ),
  ];
  for (target, route) in cases {
    assert_eq!(Route::parse(&target.path()).expect("a route"), route, "{target:?}");
  }
  let account = Target::Account(AgentAccountId::generate());
  assert!(matches!(Route::parse(&account.path()), Ok(Route::Accounts(_))));
}
