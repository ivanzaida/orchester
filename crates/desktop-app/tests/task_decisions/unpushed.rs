//! Create pull request when the seeded checkout's `main` has commits its `origin` (a local bare repository) doesn't
//! have: the Ready card asks in place and pushes nothing; Cancel puts the question away; window automation can't
//! answer it; Create anyway opens the pull request through the fake `gh` and `git push`, never pushing `main`.

use lurq::app::events::with_synthetic_input;
use serde_json::json;

use super::{
  Page, checkout_page,
  merge::{git, latest_run},
  pull_request::{FakeGitHub, READY, linked},
};

const TITLE: &str = "task-pull-request-unpushed-title";
const PERSON_ONLY: &str = "task-pull-request-unpushed-person-only";

/// Makes a bare `origin` whose `main` holds none of the local `main`'s commits (one commit of its own); how many
/// commits of the local `main` the run's branch carries, which `origin` lacks.
fn origin_behind(page: &Page) -> usize {
  let origin = page.repo.parent().expect("the seed's folder").join("origin.git");
  let origin = origin.to_str().expect("a path");
  git(page, &["init", "--quiet", "--bare", origin]);
  git(page, &["remote", "add", "origin", origin]);
  let branch = latest_run(page, READY).branch().to_string();
  let shared = out(page, &["merge-base", "main", &branch]);
  let empty = out(page, &["hash-object", "-t", "tree", "--stdin", "-w"]);
  let other = out(page, &["commit-tree", &empty, "-m", "origin's own"]);
  git(
    page,
    &["push", "--quiet", "origin", &format!("{other}:refs/heads/main")],
  );
  out(page, &["rev-list", "--count", &shared]).parse().expect("a count")
}

fn out(page: &Page, args: &[&str]) -> String {
  let output = std::process::Command::new("git")
    .arg("-C")
    .arg(&page.repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(output.status.success(), "git {args:?}");
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

/// Presses Create pull request once whether it can be used is read, until the question is shown.
fn ask(page: &mut Page) {
  page
    .window
    .until("the question about main's unpushed commits", |window| {
      if let Some(mut button) = window.tree.get_element_by_id_mut("task-create-pull-request") {
        button.click();
      }
      window.texts(TITLE).is_some()
    });
}

/// Presses `id` as an MCP client's `lurq_interact` does (lurq marks the input synthetic), until the card says that
/// only a person decides: *Create anyway* can be pressed once whether a pull request can be created is read again.
fn press_synthetic(page: &mut Page, id: &str) {
  page
    .window
    .until(&format!("{id} refused to window automation"), |window| {
      if let Some(mut button) = window.tree.get_element_by_id_mut(id) {
        with_synthetic_input(|| button.click());
      }
      window.pass();
      window.texts(PERSON_ONLY).is_some()
    });
}

#[test]
fn decision_create_pull_request_asks_before_including_unpushed_base_commits() {
  let _checkout = checkout_page();
  let fake = FakeGitHub::new();
  let mut page = linked(&fake);
  let ahead = origin_behind(&page);
  page.open(READY, "task-create-pull-request");
  ask(&mut page);
  let title = if ahead == 1 {
    "Local main has 1 commit that isn't on origin".to_owned()
  } else {
    format!("Local main has {ahead} commits that aren't on origin")
  };
  assert_eq!(page.text(TITLE), title);
  assert_eq!(
    page.text("task-pull-request-unpushed-include"),
    "This run's branch was cut from your local main, so its pull request would carry those commits too."
  );
  assert_eq!(
    page.text("task-pull-request-unpushed-push"),
    "Push main to origin first, then create the pull request. Orchester doesn't push main for you."
  );
  assert_eq!(fake.pushes(), 0, "nothing was pushed");
  assert!(fake.created.lock().is_none(), "GitHub was asked nothing");
  assert!(!page.has("task-pull-request-failure"), "it asks, it doesn't fail");
  page.click("task-pull-request-unpushed-cancel");
  page
    .window
    .until("the question to go", |window| window.texts(TITLE).is_none());
  assert_eq!(fake.pushes(), 0);
  // Window automation answers neither way.
  ask(&mut page);
  for id in ["task-pull-request-anyway", "task-pull-request-unpushed-cancel"] {
    press_synthetic(&mut page, id);
    assert_eq!(
      page.text(PERSON_ONLY),
      "Only a person decides this in the window: window automation can't create it anyway or put this away."
    );
    assert!(page.has(TITLE), "{id} by window automation leaves the question");
  }
  assert_eq!(fake.pushes(), 0);
  page.window.until("Create anyway to push the run's branch", |window| {
    if let Some(mut button) = window.tree.get_element_by_id_mut("task-pull-request-anyway") {
      button.click();
    }
    fake.pushes() > 0
  });
  page.until_says("task-pull-request-opened", "Pull request #143 opened as a draft.");
  assert_eq!(fake.pushes(), 1, "only the run's branch was pushed");
  let run = latest_run(&page, READY);
  let request = fake.request();
  assert_eq!(
    (&request["base"], &request["head"]),
    (&json!("main"), &json!(run.branch().as_str()))
  );
  let shared = out(&page, &["merge-base", "main", run.branch().as_str()]);
  assert!(
    !out(&page, &["ls-remote", "origin", "refs/heads/main"]).starts_with(&shared),
    "origin's main stays where it was"
  );
}
