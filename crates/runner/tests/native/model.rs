//! A run's own model and effort for a native step (ORC-322): chosen while the step's turn is paused, its next turn's
//! requests name them, in the same conversation.

use orchester_core::{
  id::IdempotencyKey,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{ModelId, StepKey},
  run::{Answerer, ModelChoice, ModelEvent, ModelEventKind, ModelScope, RunStatus},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use crate::{
  calls, command, events, finish, native_project,
  server::{FakeServer, Script},
  support::{native::MODEL, pending_approval, scheduler_with, wait_for},
};

/// The model the run chooses, which the endpoint lists beside the profile's.
const CHOSEN: &str = "claude-fable-5-1";

#[tokio::test(flavor = "multi_thread")]
async fn native_model_chosen_for_the_run_names_the_next_turns_requests() {
  let script = Script::new()
    .reply_sse(calls(&[("toolu_test", "run_command", command("git status"))]))
    .reply_sse(finish("toolu_finish"));
  let mut catalog = events::catalog();
  catalog.push(events::model(CHOSEN, true, 128_000, &["low", "medium", "high"]));
  let server = FakeServer::start(script, catalog, 10).await;
  let permissions = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::AskBefore, NetworkAccess::On);
  let project = native_project("Run the tests.", Some(permissions), &server);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  pending_approval(root, &mut events, id).await;
  scheduler.pause_run(id).await.expect("pausing");
  wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  let implement = StepKey::new("implement").expect("key");
  let chosen = ModelChoice {
    model: ModelId::new(CHOSEN).expect("model"),
    effort: Some("high".to_owned()),
  };
  let was = ModelChoice {
    model: ModelId::new(MODEL).expect("model"),
    effort: None,
  };
  let event = ModelEvent {
    at: Timestamp::now(),
    by: Answerer::Person,
    step: implement.clone(),
    kind: ModelEventKind::Changed {
      scope: ModelScope::Step,
      to: chosen.clone(),
      was,
      also: Vec::new(),
      before: Vec::new(),
    },
  };
  ProjectDb::open(root)
    .expect("open")
    .change_step_models(id, &[(implement, Some(chosen))], &event)
    .expect("chosen");
  scheduler.resume_run(id).await.expect("resumed");
  let run = wait_for(root, &mut events, id, |run| {
    run.status().is_terminal() || run.waits_for_a_person()
  })
  .await;
  server.finish();
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:#?}", crate::activity(root, id));

  let requests = server.model_requests();
  assert_eq!(requests.len(), 2);
  assert_eq!(requests[0].json()["model"], json!(MODEL));
  assert_eq!(requests[1].json()["model"], json!(CHOSEN));
  assert_eq!(requests[1].json()["output_config"]["effort"], json!("high"));
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations[0].session, invocations[1].session, "the same conversation");
  assert_eq!(invocations[1].model().map(ModelId::as_str), Some(CHOSEN));
}
