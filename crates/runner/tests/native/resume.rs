//! A native conversation continued (`docs/architecture/native-agent.md` §2.2, §6.2): a pause while a
//! command waits for a person ends the invocation without running it; resuming the run continues the
//! same stored conversation, the waiting call answered as not run, and the request only appends to
//! what the model was sent before.

use orchester_core::{
  connection::stream::InvocationOutcome,
  id::IdempotencyKey,
  native::session::NOT_RUN,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  run::RunStatus,
  store::ActivityStore,
};
use orchester_store::ProjectDb;

use crate::{
  calls, command, finish, native_project, serve,
  server::Script,
  support::{pending_approval, scheduler_with, wait_for},
  tool_results,
};

#[tokio::test(flavor = "multi_thread")]
async fn native_resume_continues_the_conversation_after_a_pause() {
  let script = Script::new()
    .reply_sse(calls(&[("toolu_test", "run_command", command("git status"))]))
    .expect(|request| {
      let results = tool_results(request);
      assert!(results.iter().any(|result| result.contains(NOT_RUN)), "{results:?}");
    })
    .reply_sse(finish("toolu_finish"));
  let server = serve(script).await;
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
  scheduler.resume_run(id).await.expect("resumed");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  server.finish();
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2, "{invocations:#?}");
  assert_eq!(invocations[0].outcome, Some(InvocationOutcome::Terminated));
  assert_eq!(invocations[1].outcome, Some(InvocationOutcome::Completed));
  assert!(invocations[0].session.is_some());
  assert_eq!(
    invocations[0].session, invocations[1].session,
    "one conversation for the step"
  );
}
