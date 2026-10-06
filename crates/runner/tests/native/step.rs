//! A native step run to its result: it reads and edits a file with Orchester's own tools, ends with
//! `finish_step`, and the run records what the invocation used, its conversation and its calls, never
//! the API key.

use orchester_core::{
  connection::{
    AgentProvider, BaseUrl, Effort, EngineVendor,
    stream::{AgentEvent, CallDetail, FileChangeKind, NativeCall},
  },
  id::IdempotencyKey,
  native::{store::NativeConversationStore, tools::ToolName},
  run::{InvocationEngine, RunStatus},
  store::ActivityStore,
};
use orchester_store::ProjectDb;
use serde_json::json;

use crate::{
  activity, activity_text, calls, finish, native_project, picked, serve,
  server::Script,
  support::{git, native::KEY, scheduler_with, wait_for},
};

#[tokio::test(flavor = "multi_thread")]
async fn native_step_reads_edits_and_finishes() {
  let read = json!({"path": "README.md", "offset": null, "limit": null});
  let edit = json!({"path": "README.md", "edits": [{"old": "hi\n", "new": "hi\nhello\n", "replace_all": null}]});
  let script = Script::new()
    .expect(|request| assert_eq!(request.header("x-api-key"), Some(KEY)))
    .reply_sse(calls(&[("toolu_read", "read_file", read)]))
    .reply_sse(calls(&[("toolu_edit", "edit_file", edit)]))
    .reply_sse(finish("toolu_finish"));
  let server = serve(script).await;
  let project = native_project("Say hello in the README.", None, &server);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  server.finish();
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:#?}", activity(root, id));
  let head = git(root, &["rev-parse", run.branch().as_str()]);
  assert_eq!(git(root, &["show", &format!("{head}:README.md")]), "hi\nhello");

  let entries = activity(root, id);
  let told: Vec<NativeCall> = picked(&entries, |event| match event {
    AgentEvent::NativeCall { call, .. } => Some(call.clone()),
    _ => None,
  });
  assert_eq!(told.len(), 2, "{told:#?}");
  assert_eq!(told[0].tool, ToolName::ReadFile);
  assert_eq!(
    told[0].detail,
    Some(CallDetail::Read {
      first: 1,
      last: 1,
      bytes: 3
    })
  );
  assert_eq!(
    told[1].detail,
    Some(CallDetail::Changed {
      change: FileChangeKind::Modified,
      added: 1,
      removed: 0
    })
  );
  assert!(
    !activity_text(&entries).contains(KEY),
    "the key never reaches the activity"
  );

  let db = ProjectDb::open(root).expect("open");
  let invocation = db.invocations(id).expect("invocations").remove(0);
  assert_eq!(
    db.invocation_engine(invocation.id).expect("engine"),
    Some(InvocationEngine {
      endpoint: BaseUrl::new(&server.base_url()).expect("base URL"),
      vendor: EngineVendor::Provider(AgentProvider::AnthropicApi),
      effort: Some(Effort::Low),
    })
  );
  assert!(
    invocation.usage.input >= 30,
    "usage of every response: {:?}",
    invocation.usage
  );
  let conversation = invocation
    .session
    .as_deref()
    .and_then(|session| session.parse().ok())
    .expect("the invocation names its conversation");
  let stored = db.native_conversation(conversation).expect("read").expect("stored");
  assert!(stored.system.starts_with("You are a coding agent"), "{}", stored.system);
  assert_eq!(db.native_items(conversation).expect("items").len(), 7);
  let requests = server.messages_requests();
  assert_eq!(requests.len(), 3);
  assert_eq!(requests[0].json()["output_config"]["effort"], json!("low"));
}
