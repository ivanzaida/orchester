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
  let ended = json!({"method":"turn/completed","params":{"threadId":"thread-2","turn":{"id":"turn-3","items":[],"status":"completed"}}});
  let updates = stream.parse(&ended.to_string()).expect("turn");
  assert!(matches!(
    updates.as_slice(),
    [
      Update::Activity(AgentEvent::TurnCompleted {
        usage: TokenUsage {
          input: 100,
          cached_input: 20,
          output: 30
        }
      }),
      Update::TurnEnded
    ]
  ));
}
