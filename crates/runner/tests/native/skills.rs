//! The repository's skills a native agent gets while the project has its agent files on: a search for them that
//! passes its bounds stops the agent before its first request, and says why by where the cause lies in the worktree.

use std::fs;

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::IdempotencyKey,
  run::RunStatus,
};

use crate::{
  activity, native_project, picked, serve,
  server::Script,
  support::{git, scheduler_with, wait_for},
};

#[tokio::test(flavor = "multi_thread")]
async fn native_skills_past_the_bounds_refuse_the_start() {
  let server = serve(Script::new()).await;
  let project = native_project("Say hello.", None, &server);
  let root = project.repo.path();
  let deep = root.join(".agents/skills/a/b/c/d/e/f/g/h/i");
  fs::create_dir_all(&deep).expect("nine folders deep");
  fs::write(deep.join("notes.md"), "deep\n").expect("a file");
  git(root, &["add", "."]);
  git(root, &["commit", "--quiet", "-m", "deep skills folder"]);
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::WaitingForPerson(_))
  })
  .await;
  server.finish();
  assert!(server.model_requests().is_empty(), "the agent never asks the model");
  let details: Vec<String> = picked(&activity(root, id), |event| match event {
    AgentEvent::RunnerProblem {
      problem: RunnerProblem::CouldNotStart { detail },
    } => Some(detail.clone()),
    _ => None,
  });
  let refusal = "cannot load the repository's skills: `.agents/skills/a/b/c/d/e/f/g/h/i` lies more than 8 folders \
                 deep in its skills folder";
  assert!(details.iter().any(|detail| detail == refusal), "{details:?}");
}
