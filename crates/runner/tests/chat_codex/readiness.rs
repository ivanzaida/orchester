//! Readiness must progress in the chat driver as well as the step worker.
use super::*;

fn with_model(model: &str) -> TestProject {
  let project = codex();
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  let connection = db.connections().expect("connections")[0].id();
  db.publish_orchestrator(
    &OrchestratorSettings::new(connection, ModelId::new(model).expect("model"), None),
    Timestamp::now(),
  )
  .expect("settings");
  project
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_mcp_readiness_connects_before_fresh_and_resumed_turns() {
  let project = with_model("mcp-ready");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  for text in ["First message.", "Second message."] {
    let message = keep(root, chat, text);
    scheduler.answer_in_chat(message, tools()).await.expect("start");
    assert_eq!(ended(root, &mut events, message).await, TurnState::Answered);
  }
  let logged = log(root);
  let states: Vec<_> = logged
    .lines()
    .filter(|line| line.starts_with("mcp-readiness:") || line.starts_with("prompt:"))
    .collect();
  assert_eq!(
    states,
    [
      "mcp-readiness: starting",
      "mcp-readiness: connected",
      "prompt: First message.",
      "mcp-readiness: starting",
      "mcp-readiness: connected",
      "prompt: Second message.",
    ]
  );
  assert!(logged.contains("thread: fake-thread-1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_mcp_readiness_failure_is_redacted_and_starts_no_turn() {
  let project = with_model("mcp-ready-failed");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let message = keep(root, chat, "Say hello.");
  scheduler.answer_in_chat(message, tools()).await.expect("start");
  let state = ended(root, &mut events, message).await;
  assert!(
    matches!(&state, TurnState::Failed(TurnFailure::Failed { reason })
    if reason.contains("`orchester`") && reason.contains("failed: startup refused token <redacted> second line")),
    "{state:?}"
  );
  assert!(!format!("{state:?}").contains(TOKEN));
  assert!(!log(root).contains("prompt:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_mcp_readiness_deadline_ends_an_unanswered_status() {
  let project = with_model("mcp-ready-silent");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let message = keep(root, ChatId::generate(), "Say hello.");
  scheduler.answer_in_chat(message, tools()).await.expect("start");
  let state = tokio::time::timeout(Duration::from_secs(75), async {
    loop {
      let state = ProjectDb::open(root)
        .expect("open")
        .chat_turn(message)
        .expect("turn")
        .expect("kept")
        .turn;
      if !state.is_thinking() {
        break state;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("bounded readiness");
  assert!(
    matches!(&state, TurnState::Failed(TurnFailure::Failed { reason })
    if reason.contains("Timed out waiting") && reason.contains("`orchester`") && reason.ends_with("starting")),
    "{state:?}"
  );
  assert!(!log(root).contains("prompt:"));
}
