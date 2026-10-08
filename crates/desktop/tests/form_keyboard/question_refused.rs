//! A question for a secret that Orchester did not put to the person (ORC-83, C·17 state H): a problem row in
//! the chat names what was asked, says why it was declined and what to do instead, and *Open Secrets ↗*
//! opens the project's Secrets. Seeded, without a runner.
//! `cargo test -p orchester-desktop --test form_keyboard question_refused`.

use orchester_core::{
  connection::stream::{AgentEvent, QuestionRefusal, RunnerProblem},
  store::ActivityStore,
};
use orchester_store::ProjectDb;

use crate::{github::contains_text, harness::Desktop, native_run::native_run, unreadable_run::at};

/// The id of the first element whose id starts with `prefix`.
fn find_id(element: lurq::node::element::ElementRef<'_>, prefix: &str) -> Option<String> {
  if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
    return Some(id.to_owned());
  }
  element.children().into_iter().find_map(|child| find_id(child, prefix))
}

#[test]
fn question_refused_names_the_secret_and_opens_secrets() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let (_, invocation) = native_run(repo);
    let problem = RunnerProblem::QuestionRefused {
      question: "GitHub client secret".to_owned(),
      reason: QuestionRefusal::Secret,
    };
    ProjectDb::open(repo)
      .expect("open the project")
      .append_activity(&invocation, &[(at(55), AgentEvent::RunnerProblem { problem })])
      .expect("the refusal");
  });
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-page");
  desktop.click("task-tab-conversation");
  let link = |desktop: &mut Desktop| {
    desktop
      .tree
      .root()
      .and_then(|root| find_id(root, "activity-open-secrets-"))
  };
  desktop.wait_until("the refusal's row", |desktop| link(desktop).is_some());
  let link = link(&mut desktop).expect("the link");
  for text in [
    "Asked for a secret: GitHub client secret",
    "refused",
    "Orchester never passes secret values through questions",
    "Open Secrets ↗",
  ] {
    let row = desktop.tree.find_element(|element| {
      element.id().is_some_and(|id| id.starts_with("conversation-row:")) && contains_text(element, text)
    });
    assert!(row.is_some(), "{text}");
  }
  desktop.click(&link);
  desktop.wait_for("secrets-import");
}
