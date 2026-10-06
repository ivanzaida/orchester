//! A native agent's commands that ask a person (`docs/architecture/native-agent.md` §5.2): the approval
//! records the command with its time limit, Approve once runs it, and a denial is the call's result:
//! the agent continues and the step passes (decision 2).

use orchester_core::{
  connection::stream::{AgentEvent, DeclinedBy},
  id::IdempotencyKey,
  native::session::DENIED,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{ApprovalAnswer, DecidedBy, NativeCommandAsk, RunStatus},
};

use crate::{
  activity, calls, command, finish, native_project, picked, serve,
  server::Script,
  support::{pending_approval, scheduler_with, wait_for},
  tool_results,
};

#[tokio::test(flavor = "multi_thread")]
async fn native_approvals_run_once_and_a_denial_continues() {
  let script = Script::new()
    .reply_sse(calls(&[("toolu_status", "run_command", command("git status --short"))]))
    .reply_sse(calls(&[("toolu_log", "run_command", command("git log -1"))]))
    .expect(|request| {
      let results = crate::tool_results(request);
      assert!(results.iter().any(|result| result.contains(DENIED)), "{results:?}");
    })
    .reply_sse(finish("toolu_finish"));
  let server = serve(script).await;
  let permissions = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::AskBefore, NetworkAccess::On);
  let project = native_project("Look at git.", Some(permissions), &server);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");

  let request = pending_approval(root, &mut events, id).await;
  assert!(request.command);
  assert_eq!(request.action, "git status --short");
  assert!(request.id.ends_with(":toolu_status"), "{}", request.id);
  assert_eq!(
    request.native,
    Some(NativeCommandAsk {
      shell: None,
      time_limit_seconds: 120
    })
  );
  tokio::time::sleep(std::time::Duration::from_millis(30)).await;
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once)
    .await
    .expect("approved");

  let request = pending_approval(root, &mut events, id).await;
  assert_eq!(request.action, "git log -1");
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Deny)
    .await
    .expect("denied");

  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  server.finish();
  assert_eq!(run.status(), &RunStatus::Succeeded, "a denial does not fail the step");
  let entries = activity(root, id);
  let decided: Vec<(DecidedBy, Option<u64>)> = picked(&entries, |event| match event {
    AgentEvent::ToolDecided { decision, .. } => Some((decision.decided_by, decision.waited_ms)),
    _ => None,
  });
  assert_eq!(decided.len(), 2, "{decided:?}");
  assert_eq!(decided[0].0, DecidedBy::Person);
  assert!(decided[0].1.is_some_and(|waited| waited >= 30), "{decided:?}");
  assert_eq!(decided[1].0, DecidedBy::PersonDeny);
  let declined: Vec<DeclinedBy> = picked(&entries, |event| match event {
    AgentEvent::NativeCall { call, .. } => call.declined.as_ref().map(|declined| declined.by),
    _ => None,
  });
  assert_eq!(declined, [DeclinedBy::Person]);
  let ran: Vec<Option<i64>> = picked(&entries, |event| match event {
    AgentEvent::CommandFinished { command, exit_code, .. } if command == "git status --short" => Some(*exit_code),
    _ => None,
  });
  assert_eq!(ran, [Some(0)], "the approved command ran once");
  let first = &server.messages_requests()[1];
  assert!(tool_results(first).iter().any(|result| result.contains("exit code 0")));
}
