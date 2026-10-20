//! Returning the work of an ended run (the project task ORC-44 · Return a finished run to an earlier step): *Ask an
//! agent to resolve* on a merge that conflicted returns the run to the step the person picks among those whose agent
//! may write, with a note that says what to merge and how, and the runner starts a new attempt from it;
//! `continue_run`'s `return_to` takes an ended run under the same rules and refuses a merged run, one whose worktree
//! was removed and one that still works.

use std::{fs, time::Duration};

use orchester_core::{
  run::{AttemptReason, Run},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{
  Served,
  kontur::{self, SeedRequest, Seeded, scenarios},
};

/// The latest run of the task seeded in `state`, and the task's key.
pub(super) fn latest(seeded: &Seeded, state: &str) -> (String, Run) {
  let task = seeded
    .tasks
    .iter()
    .find(|task| task.state == state)
    .unwrap_or_else(|| panic!("a task {state}"));
  let db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let id = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|candidate| candidate.key().to_string() == task.key)
    .expect("the task")
    .id();
  let run = db
    .runs_of_task(id)
    .expect("runs")
    .pop()
    .expect("its run")
    .expect("readable");
  (task.key.clone(), run)
}

fn return_to(run: &Run, step: &str) -> Value {
  json!({
    "project": "Kontur",
    "run": run.number().get().to_string(),
    "choice": { "kind": "return_to", "step": step, "note": "Merge main into this branch." },
  })
}

#[test]
fn domain_ask_an_agent_returns_the_conflicting_run_to_its_writing_step() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let seeded = kontur::seed(&SeedRequest {
    data_dir: folder.path().join("bin"),
    repo: folder.path().join("repo-decisions"),
    scenario: scenarios::named("decisions").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  })
  .expect("the scenario seeds");
  let (key, run) = latest(&seeded, "ready for review, conflicts with main");
  // The real runner, which records the return and starts the step's attempt.
  let runner = super::runner_programs::programs().runner.clone();
  let mut served = Served::start_with(folder, &seeded.app_database, runner, true);
  served.ok(
    "navigate",
    &json!({ "route": "task", "project": seeded.project.to_string(), "key": key, "tab": "summary" }),
  );
  served.until("Merge into main", |served| served.shows("task-merge"));
  served.click_by_person("task-merge");
  served.until("the conflict and the agent's step", |served| {
    served.shows("task-resolve-step") && served.shows("task-resolve-ask-agent")
  });
  let options: Vec<String> = served
    .tree
    .get_element_by_id_mut("task-resolve-step")
    .and_then(|element| element.as_select())
    .expect("the step select")
    .labels()
    .iter()
    .map(ToString::to_string)
    .collect();
  assert_eq!(
    options,
    ["Implement · Builder"],
    "only the step that may write is offered"
  );
  // Returning a run decides for the person: an MCP client's press is refused until a person lets automation answer,
  // as `continue_run` is, and then recorded as automation's.
  served
    .click_by_client("task-resolve-ask-agent")
    .expect("the press reaches the button");
  served.until("the refusal", |served| served.shows("task-resolve-person-only"));
  assert_eq!(
    stored(&seeded, run.id()).attempts().len(),
    run.attempts().len(),
    "nothing was returned"
  );
  let_automation_answer(&served);
  served
    .click_by_client("task-resolve-ask-agent")
    .expect("the press reaches the button");
  let returned = wait_for(&mut served, &seeded, run.id(), |stored| {
    stored.attempts().len() > run.attempts().len()
  });
  let AttemptReason::ReturnedAfterEnd {
    return_to,
    note,
    conflicts,
  } = returned.attempts()[run.attempts().len()].reason().clone()
  else {
    panic!("a return after the run ended: {:?}", returned.attempts().last());
  };
  assert_eq!(return_to.as_str(), "implement");
  assert!(
    note
      .as_str()
      .starts_with("Merge main into this branch and resolve the conflicts listed below"),
    "{}",
    note.as_str()
  );
  assert!(note.as_str().ends_with("commit the merge."), "{}", note.as_str());
  assert!(
    !note.as_str().contains("src/"),
    "the files are data beside the note: {}",
    note.as_str()
  );
  let conflicts = conflicts.expect("what the return resolves");
  assert_eq!(conflicts.base.as_str(), "main");
  assert_eq!(conflicts.files, ["src/webhooks/sender.rs", "src/webhooks/settings.rs"]);
}

#[test]
fn domain_window_input_merges_into_the_worktree_but_starts_no_editor_a_person_did_not_name() {
  let (folder, seeded) = Served::seed("decisions", None);
  let (key, run) = latest(&seeded, "ready for review, conflicts with main");
  let runner = folder.path().join("no-runner").join("orchester-runner");
  let mut served = Served::start_with(folder, &seeded.app_database, runner, true);
  served.ok(
    "navigate",
    &json!({ "route": "task", "project": seeded.project.to_string(), "key": key, "tab": "summary" }),
  );
  served.until("Merge into main", |served| served.shows("task-merge"));
  served.click_by_person("task-merge");
  served.until("the conflict", |served| served.shows("task-resolve-in-editor"));
  served
    .click_by_client("task-resolve-in-editor")
    .expect("the press reaches the button");
  served.until("the merge waiting in the worktree", |served| {
    super::tools_window::text(served, "task-resolving-body").contains("Window input does not start an editor")
  });
  let worktree = seeded.repo.join(".orchester/worktrees").join(run.id().to_string());
  assert!(
    std::process::Command::new("git")
      .arg("-C")
      .arg(&worktree)
      .args(["rev-parse", "--quiet", "--verify", "MERGE_HEAD"])
      .status()
      .expect("git")
      .success(),
    "the merge waits in the worktree"
  );
}

/// A person turns on *Let automation answer approvals*.
fn let_automation_answer(served: &Served) {
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("a person lets automation answer");
}

fn stored(seeded: &Seeded, run: orchester_core::id::RunId) -> Run {
  ProjectDb::open(&seeded.repo)
    .expect("the database opens")
    .run(run)
    .expect("read")
    .expect("the run")
}

/// The run as stored once `done` holds for it, the window drawn meanwhile.
pub(super) fn wait_for(
  served: &mut Served,
  seeded: &Seeded,
  run: orchester_core::id::RunId,
  done: impl Fn(&Run) -> bool,
) -> Run {
  for _ in 0..600 {
    served.pass();
    let stored = ProjectDb::open(&seeded.repo)
      .expect("the database opens")
      .run(run)
      .expect("read")
      .expect("the run");
    if done(&stored) {
      return stored;
    }
    std::thread::sleep(Duration::from_millis(100));
  }
  panic!("the run did not change in time");
}

#[test]
fn domain_continue_run_returns_only_an_ended_run_that_was_not_merged_and_has_its_worktree() {
  let (mut served, seeded) = Served::seeded("task-states");
  let (_, merged) = latest(&seeded, "merged");
  assert_eq!(
    served.refused("continue_run", &return_to(&merged, "implement"))["code"],
    "automation_not_allowed",
    "returning a run decides for the person"
  );
  let_automation_answer(&served);
  assert_eq!(
    served.refused("continue_run", &return_to(&merged, "implement"))["code"],
    "run_merged"
  );
  let (_, running) = latest(&seeded, "running");
  assert_eq!(
    served.refused("continue_run", &return_to(&running, "implement"))["code"],
    "run_not_ended"
  );

  let (mut served, seeded) = Served::seeded("decisions");
  let_automation_answer(&served);
  let (_, ready) = latest(&seeded, "ready for review, merges cleanly");
  assert_eq!(
    served.refused("continue_run", &return_to(&ready, "deploy"))["code"],
    "step_not_offered"
  );
  // Past the rules the return goes to the runner, which this app has none of.
  let sent = served.refused("continue_run", &return_to(&ready, "Implement"));
  assert!(
    !["run_merged", "run_not_ended", "worktree_removed", "step_not_offered"]
      .contains(&sent["code"].as_str().unwrap_or("")),
    "{sent}"
  );
  let worktree = seeded.repo.join(".orchester/worktrees").join(ready.id().to_string());
  fs::remove_dir_all(&worktree).expect("the worktree's folder removed");
  assert_eq!(
    served.refused("continue_run", &return_to(&ready, "implement"))["code"],
    "worktree_removed"
  );
}
