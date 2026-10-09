use orchester_core::{
  connection::{
    app_server::{AppServerStream, ProtocolError, Update},
    stream::AgentEvent,
  },
  run::TokenUsage,
};
use serde_json::{Value, json};

fn approval(updates: &[Update]) -> orchester_core::connection::app_server::PendingApproval {
  let [Update::Approval(request)] = updates else {
    panic!("one approval request expected");
  };
  request.clone()
}

#[test]
fn one_app_tool_decision_is_scoped_to_its_item_and_keeps_arguments_out_of_debug() {
  let mut stream = AppServerStream::default();
  let started = json!({"method":"item/started","params":{"threadId":"thread-1","turnId":"turn-1","item":{
    "id":"tool-7","type":"mcpToolCall","server":"pencil","tool":"batch_design",
    "arguments":{"filePath":"/scratch/disposable.pen","operations":"secret-shaped arbitrary content"},
    "status":"inProgress"
  }}});
  assert!(stream.parse(&started.to_string()).expect("item").is_empty());
  let requested = json!({"id":42,"method":"item/tool/requestUserInput","params":{
    "threadId":"thread-1","turnId":"turn-1","itemId":"tool-7","isBlocking":true,
    "questions":[{"header":"Approval","id":"action","question":"Approve this write?","options":[
      {"label":"Accept","description":"Run this tool once"},
      {"label":"Decline","description":"Do not run it"}
    ]}]
  }});
  let request = approval(&stream.parse(&requested.to_string()).expect("approval"));
  assert_eq!(request.server, "pencil");
  assert_eq!(request.action, "batch_design");
  assert_eq!(request.target.as_deref(), Some("/scratch/disposable.pen"));
  assert!(!format!("{request:?}").contains("secret-shaped"));
  assert_eq!(
    serde_json::from_str::<Value>(&request.response(true)).expect("response"),
    json!({"id":42,"result":{"answers":{"action":{"answers":["Accept"]}}}})
  );
  assert_eq!(
    serde_json::from_str::<Value>(&request.response(false)).expect("response"),
    json!({"id":42,"result":{"answers":{"action":{"answers":["Decline"]}}}})
  );
  let mut documented_method = requested;
  documented_method["method"] = json!("tool/requestUserInput");
  let equivalent = approval(
    &stream
      .parse(&documented_method.to_string())
      .expect("documented request"),
  );
  assert_eq!(equivalent, request);
}

#[test]
fn pending_approval_does_not_hide_other_active_tools() {
  let mut stream = AppServerStream::default();
  for item in ["waiting", "executing"] {
    let started = json!({"method":"item/started","params":{"threadId":"t","turnId":"turn","item":{
      "id":item,"type":"mcpToolCall","server":"pencil","tool":"batch_design"
    }}});
    stream.parse(&started.to_string()).expect("start");
  }
  let requested = json!({"id":42,"method":"item/tool/requestUserInput","params":{
    "threadId":"t","turnId":"turn","itemId":"waiting","isBlocking":true,
    "questions":[{"id":"action","options":[{"label":"Accept"},{"label":"Decline"}]}]
  }});
  let pending = approval(&stream.parse(&requested.to_string()).expect("approval"));
  assert_eq!(pending.item(), "waiting");
  assert!(stream.has_other_active_tools(&pending));
  stream
    .parse(
      &json!({"method":"item/completed","params":{"item":{
        "id":"executing","type":"mcpToolCall"
      }}})
      .to_string(),
    )
    .expect("completion");
  assert!(stream.has_active_tools(), "the waiting item is still present");
  assert!(!stream.has_other_active_tools(&pending));
}

#[test]
fn empty_mcp_confirmation_accepts_once_and_required_form_fails_closed() {
  let mut stream = AppServerStream::default();
  let started = json!({"method":"item/started","params":{"threadId":"thread-2","turnId":"turn-3","item":{
    "id":"tool-9","type":"mcpToolCall","server":"pencil","tool":"batch_design",
    "arguments":{"filePath":"/scratch/disposable.pen"}
  }}});
  assert!(stream.parse(&started.to_string()).expect("item").is_empty());
  let requested = json!({"id":"rpc-9","method":"mcpServer/elicitation/request","params":{
    "threadId":"thread-2","turnId":"turn-3","serverName":"pencil","mode":"form",
    "message":"Allow this change?","requestedSchema":{"type":"object","properties":{}}
  }});
  let request = approval(&stream.parse(&requested.to_string()).expect("approval"));
  assert_eq!(
    serde_json::from_str::<Value>(&request.response(true)).expect("response"),
    json!({"id":"rpc-9","result":{"action":"accept","content":{},"_meta":null}})
  );
  assert_eq!(
    serde_json::from_str::<Value>(&request.response(false)).expect("response"),
    json!({"id":"rpc-9","result":{"action":"decline","content":null,"_meta":null}})
  );
  let mut required = requested;
  required["params"]["requestedSchema"]["required"] = json!(["credential"]);
  assert_eq!(
    stream.parse(&required.to_string()).err(),
    Some(ProtocolError::InvalidApproval)
  );
}

#[test]
fn mcp_confirmation_without_an_item_id_rejects_ambiguous_parallel_calls() {
  let mut stream = AppServerStream::default();
  for item in ["tool-1", "tool-2"] {
    let started = json!({"method":"item/started","params":{"threadId":"thread-1","turnId":"turn-1","item":{
      "id":item,"type":"mcpToolCall","server":"pencil","tool":"batch_design"
    }}});
    assert!(stream.parse(&started.to_string()).expect("item").is_empty());
  }
  let request = json!({"id":99,"method":"mcpServer/elicitation/request","params":{
    "threadId":"thread-1","turnId":"turn-1","serverName":"pencil","mode":"form",
    "message":"Allow this change?","requestedSchema":{"type":"object","properties":{}}
  }});
  assert_eq!(
    stream.parse(&request.to_string()).err(),
    Some(ProtocolError::InvalidApproval)
  );
}

#[test]
fn user_input_rejects_secret_other_and_nonblocking_questions() {
  let mut stream = AppServerStream::default();
  let started = json!({"method":"item/started","params":{"threadId":"thread-1","turnId":"turn-1","item":{
    "id":"tool-1","type":"mcpToolCall","server":"pencil","tool":"batch_design"
  }}});
  assert!(stream.parse(&started.to_string()).expect("item").is_empty());
  let request = json!({"id":7,"method":"item/tool/requestUserInput","params":{
    "threadId":"thread-1","turnId":"turn-1","itemId":"tool-1","isBlocking":true,
    "questions":[{"header":"Approval","id":"action","question":"Approve?","options":[
      {"label":"Accept","description":"Once"},{"label":"Decline","description":"Deny"}
    ]}]
  }});
  for (field, value) in [
    ("isBlocking", json!(false)),
    ("isSecret", json!(true)),
    ("isOther", json!(true)),
  ] {
    let mut variant = request.clone();
    if field == "isBlocking" {
      variant["params"][field] = value;
    } else {
      variant["params"]["questions"][0][field] = value;
    }
    assert_eq!(
      stream.parse(&variant.to_string()).err(),
      Some(ProtocolError::InvalidApproval)
    );
  }
}

#[test]
fn thread_notification_does_not_duplicate_the_session_start_response() {
  let mut stream = AppServerStream::default();
  let started = json!({"method":"thread/started","params":{"thread":{"id":"thread-1"}}});
  assert!(
    stream
      .parse(&started.to_string())
      .expect("thread notification")
      .is_empty()
  );
  let turn = json!({"method":"turn/started","params":{"threadId":"thread-1","turn":{"id":"turn-1"}}});
  assert!(matches!(
    stream.parse(&turn.to_string()).expect("turn notification").as_slice(),
    [Update::Activity(AgentEvent::TurnStarted)]
  ));
}

#[test]
fn unknown_server_request_fails_closed_and_turn_usage_is_counted_once() {
  let mut stream = AppServerStream::default();
  let unknown = json!({"id":5,"method":"item/permissions/requestApproval","params":{}});
  assert_eq!(
    stream.parse(&unknown.to_string()).err(),
    Some(ProtocolError::UnsupportedRequest {
      method: "item/permissions/requestApproval".to_owned()
    })
  );
  let usage = json!({"method":"thread/tokenUsage/updated","params":{"threadId":"thread-2","turnId":"turn-3",
    "tokenUsage":{"last":{"inputTokens":100,"cachedInputTokens":20,"outputTokens":30,"reasoningOutputTokens":10,"totalTokens":130},
      "total":{"inputTokens":100,"cachedInputTokens":20,"outputTokens":30,"reasoningOutputTokens":10,"totalTokens":130}}
  }});
  assert!(stream.parse(&usage.to_string()).expect("usage").is_empty());
  // Repeated unchanged, as Codex does when its rate limits change: counted once.
  assert!(stream.parse(&usage.to_string()).expect("usage").is_empty());
  // A second model response of the same turn adds its own usage.
  let second = json!({"method":"thread/tokenUsage/updated","params":{"threadId":"thread-2","turnId":"turn-3",
    "tokenUsage":{"last":{"inputTokens":50,"cachedInputTokens":40,"outputTokens":5,"reasoningOutputTokens":0,"totalTokens":55},
      "total":{"inputTokens":150,"cachedInputTokens":60,"outputTokens":35,"reasoningOutputTokens":10,"totalTokens":185}}
  }});
  assert!(stream.parse(&second.to_string()).expect("usage").is_empty());
  let ended = json!({"method":"turn/completed","params":{"threadId":"thread-2","turn":{"id":"turn-3","items":[],"status":"completed"}}});
  let updates = stream.parse(&ended.to_string()).expect("turn");
  assert!(matches!(
    updates.as_slice(),
    [
      Update::Activity(AgentEvent::TurnCompleted {
        usage: TokenUsage {
          input: 150,
          cached_input: 60,
          output: 35
        }
      }),
      Update::TurnEnded { input_kept: true }
    ]
  ));
}

#[test]
fn an_interrupted_turn_does_not_keep_the_input_it_accepted() {
  // Codex records accepted `turn/steer` input as a turn completes or fails, and holds it back for a
  // turn of its own when the turn is interrupted.
  for (status, kept) in [("completed", true), ("failed", true), ("interrupted", false)] {
    let mut stream = AppServerStream::default();
    let ended = json!({"method":"turn/completed","params":{"threadId":"thread-1","turn":{"id":"turn-1","items":[],"status":status}}});
    let updates = stream.parse(&ended.to_string()).expect("turn");
    assert!(
      matches!(updates.last(), Some(Update::TurnEnded { input_kept }) if *input_kept == kept),
      "{status}"
    );
  }
}

#[test]
fn a_completed_patch_reports_the_files_it_changed() {
  use orchester_core::connection::stream::{FileChange, FileChangeKind};
  let mut stream = AppServerStream::default();
  let completed = json!({"method":"item/completed","params":{"threadId":"thread-1","turnId":"turn-1","item":{
    "id":"patch-3","type":"fileChange","status":"completed","changes":[
      {"path":"src/auth/github.ts","kind":{"type":"add"},"diff":"+export {}"},
      {"path":"src/auth/providers.ts","kind":"update","diff":"-a\n+b"}
    ]
  }}});
  let updates = stream.parse(&completed.to_string()).expect("item");
  let [Update::Activity(AgentEvent::FileChanged { item, changes })] = &updates[..] else {
    panic!("one file change expected");
  };
  assert_eq!(item, "patch-3");
  assert_eq!(
    changes,
    &[
      FileChange {
        path: "src/auth/github.ts".into(),
        kind: FileChangeKind::Added
      },
      FileChange {
        path: "src/auth/providers.ts".into(),
        kind: FileChangeKind::Modified
      },
    ]
  );
  let declined = json!({"method":"item/completed","params":{"item":{
    "id":"patch-4","type":"fileChange","status":"declined","changes":[{"path":"a.rs","kind":"update"}]
  }}});
  assert!(stream.parse(&declined.to_string()).expect("item").is_empty());
}

#[test]
fn a_user_message_the_turn_took_is_reported_with_its_text() {
  // Codex 0.159.2 (probe 2026-10-02): a message added with `turn/steer` while a command ran became a
  // `userMessage` item right after the command completed.
  let mut stream = AppServerStream::default();
  let taken = json!({"method":"item/completed","params":{"threadId":"thread-1","turnId":"turn-1","item":{
    "id":"item-9","type":"userMessage","content":[{"type":"text","text":"Also cover cancel.","text_elements":[]}]
  }}});
  let updates = stream.parse(&taken.to_string()).expect("item");
  assert!(matches!(updates.as_slice(), [Update::InputTaken(text)] if text == "Also cover cancel."));
}

/// Codex runs Orchester's tools without asking, so the runner asks a person itself; the call's end then says a person
/// was asked, whether the runner's request comes before Codex starts the call or after.
#[test]
fn app_server_a_call_the_runner_asked_about_ends_as_asked() {
  let input = json!({"filename": "/deps/notes.txt"});
  let call = |id: &str, arguments: &Value| {
    let item = json!({"id": id, "type": "mcpToolCall", "server": "Orchester", "tool": "read_external_file",
      "arguments": arguments});
    let mut done = item.clone();
    done["status"] = json!("completed");
    (
      json!({"method": "item/started", "params": {"threadId": "t", "turnId": "turn", "item": item}}).to_string(),
      json!({"method": "item/completed", "params": {"threadId": "t", "turnId": "turn", "item": done}}).to_string(),
    )
  };
  let asked = |updates: Vec<Update>| {
    updates.into_iter().find_map(|update| match update {
      Update::Activity(AgentEvent::McpToolFinished { item, asked, .. }) => Some((item, asked)),
      _ => None,
    })
  };
  let mut stream = AppServerStream::default();
  let (started, completed) = call("mcp-1", &input);
  stream.parse(&started).expect("started");
  assert_eq!(
    stream.runner_asks("read_external_file", &input).as_deref(),
    Some("mcp-1")
  );
  assert_eq!(
    asked(stream.parse(&completed).expect("completed")),
    Some(("mcp-1".to_owned(), true))
  );
  // Asked before Codex started the call: its start is marked.
  assert_eq!(stream.runner_asks("read_external_file", &input), None);
  let (started, completed) = call("mcp-2", &input);
  stream.parse(&started).expect("started");
  assert_eq!(
    asked(stream.parse(&completed).expect("completed")),
    Some(("mcp-2".to_owned(), true))
  );
  // A call with other arguments was not asked about.
  let (started, completed) = call("mcp-3", &json!({"filename": "/deps/other.txt"}));
  stream.parse(&started).expect("started");
  assert_eq!(
    asked(stream.parse(&completed).expect("completed")),
    Some(("mcp-3".to_owned(), false))
  );
}
