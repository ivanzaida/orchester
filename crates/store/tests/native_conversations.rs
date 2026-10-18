//! Native conversations (`docs/architecture/native-agent.md` §6.2): stored once, their items appended in
//! order with their segments, never changed; and the engine a native invocation used.
//!
//! `cargo test -p orchester-store --test native_conversations native_`

mod support;

use orchester_core::{
  connection::{AgentProvider, BaseUrl, Effort, EngineVendor, WireApi},
  id::{ConnectionId, ConversationId},
  native::{
    item::{Item, ToolCallId, UserOrigin},
    store::{NativeConversation, NativeConversationStore},
    tools::ToolName,
  },
  permission::RepositoryAccess,
  run::InvocationEngine,
  store::ActivityStore,
};
use rusqlite::Connection;
use serde_json::json;

use self::support::{at, model, started_run};

#[test]
fn native_conversation_round_trips_with_its_items() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  let conversation = NativeConversation {
    id: ConversationId::generate(),
    run: run.id(),
    execution,
    connection: ConnectionId::generate(),
    wire: WireApi::AnthropicMessages,
    model: model(),
    system: "You are a coding agent.".to_owned(),
    access: RepositoryAccess::ReadOnly,
    created_at: at(10),
  };
  db.create_native_conversation(&conversation).expect("create");
  assert_eq!(
    db.native_conversation(conversation.id).expect("read").as_ref(),
    Some(&conversation)
  );
  assert!(!conversation.tools().offers(ToolName::EditFile));
  assert_eq!(db.native_conversation(ConversationId::generate()).expect("read"), None);

  let call = ToolCallId::new("toolu_1").expect("id");
  let items = vec![
    Item::user("Do the task.", UserOrigin::Task),
    Item::ToolCall {
      id: call.clone(),
      name: "read_file".to_owned(),
      input: json!({"path": "README.md"}),
    },
    Item::tool_result(call, "1\thi", false),
  ];
  db.append_native_items(conversation.id, invocation.id, &items[..2])
    .expect("append");
  db.append_native_items(conversation.id, invocation.id, &items[2..])
    .expect("append");
  let compacted = vec![
    Item::Compaction {
      summary: "Read the readme.".to_owned(),
    },
    Item::user("Read files again.", UserOrigin::Compaction),
  ];
  db.append_native_items(conversation.id, invocation.id, &compacted)
    .expect("append");
  assert_eq!(
    db.native_items(conversation.id).expect("read"),
    [items, compacted].concat()
  );

  let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("open the database");
  let segments: Vec<(i64, i64)> = raw
    .prepare("SELECT seq, segment FROM native_items ORDER BY seq")
    .expect("query")
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
    .expect("rows")
    .collect::<Result<_, _>>()
    .expect("read");
  assert_eq!(segments, [(1, 0), (2, 0), (3, 0), (4, 1), (5, 1)]);
  for statement in [
    "UPDATE native_items SET body = '{}' WHERE seq = 1",
    "DELETE FROM native_items WHERE seq = 5",
    "UPDATE native_conversations SET system = 'changed'",
  ] {
    assert!(raw.execute(statement, []).is_err(), "{statement}");
  }
}

#[test]
fn native_invocation_engine_is_kept_once() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  assert_eq!(db.invocation_engine(invocation.id).expect("read"), None);
  let engine = InvocationEngine {
    endpoint: BaseUrl::new("https://api.anthropic.com").expect("base URL"),
    vendor: EngineVendor::Provider(AgentProvider::AnthropicApi),
    effort: Some(Effort::High),
  };
  db.save_invocation_engine(invocation.id, &engine).expect("save");
  assert_eq!(db.invocation_engine(invocation.id).expect("read"), Some(engine.clone()));
  assert!(db.save_invocation_engine(invocation.id, &engine).is_err());
}
