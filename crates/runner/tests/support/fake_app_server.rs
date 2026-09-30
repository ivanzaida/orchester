//! Protocol fixture: side effects happen only after the exact one-action response.

use std::{
  collections::BTreeSet,
  fs,
  io::{self, BufRead, Write},
  path::PathBuf,
  process::ExitCode,
};

use serde_json::{Value, json};

fn emit(value: &Value) {
  println!("{value}");
  io::stdout().flush().expect("flush protocol");
}

pub fn run(args: &[String]) -> ExitCode {
  let mut cwd = PathBuf::new();
  let mut thread = String::new();
  let turn = "approval-turn";
  let mut stage = 0;
  let mut hold_approved = false;
  let mut roots = BTreeSet::<String>::new();
  for line in io::stdin().lock().lines() {
    let message: Value = serde_json::from_str(&line.expect("stdin")).expect("JSON RPC");
    let params = &message["params"];
    match message["method"].as_str() {
      Some("initialize") => emit(&json!({"id":1,"result":{}})),
      Some("initialized") => {}
      Some("account/read") => emit(&json!({"id":2,"result":{"account":{"type":"chatgpt"}}})),
      Some("thread/start" | "thread/resume") => {
        cwd = PathBuf::from(params["cwd"].as_str().expect("cwd"));
        thread = params["threadId"]
          .as_str()
          .unwrap_or("fake-interactive-thread")
          .to_owned();
        roots = thread_response(args, params, &cwd, &thread);
      }
      Some("mcpServerStatus/list") => emit(&json!({"id":4,"result":{"data":[],"nextCursor":null}})),
      Some("turn/start") => {
        assert_eq!(
          params["environments"],
          json!([{"environmentId":"local","cwd":cwd,"runtimeWorkspaceRoots":roots}])
        );
        hold_approved = params["input"][0]["text"]
          .as_str()
          .expect("prompt")
          .contains("Hold approved command");
        emit(&json!({"id":5,"result":{"turn":{"id":turn}}}));
        emit(&json!({"method":"turn/started","params":{"threadId":thread,"turn":{"id":turn}}}));
        command_request(&thread, turn, &cwd);
      }
      None if message["id"] == 100 && stage == 0 => {
        assert_eq!(
          message["result"],
          json!({"decision":"accept"}),
          "only an exact action grant"
        );
        fs::write(cwd.join("command-once.txt"), "approved").expect("command side effect");
        if hold_approved {
          stage = 2;
          continue;
        }
        emit(&json!({"method":"item/completed","params":{"item":{
          "id":"command-1","type":"commandExecution","command":"disposable-command",
          "status":"completed","exitCode":0,"aggregatedOutput":""
        }}}));
        stage = 1;
        tool_request(&thread, turn);
      }
      None if message["id"] == 101 && stage == 1 => {
        assert_eq!(message["result"]["action"], "accept");
        fs::write(cwd.join("mcp-once.txt"), "approved").expect("MCP side effect");
        emit(&json!({"method":"item/completed","params":{"item":{
          "id":"message-1","type":"agentMessage","text":"{\"outcome\":\"done\",\"summary\":\"continued\"}"
        }}}));
        emit(&json!({"method":"turn/completed","params":{"turn":{"id":turn,"status":"completed"}}}));
        return ExitCode::SUCCESS;
      }
      _ => return ExitCode::from(2),
    }
  }
  ExitCode::SUCCESS
}

fn thread_response(args: &[String], params: &Value, cwd: &std::path::Path, thread: &str) -> BTreeSet<String> {
  let network = args
    .iter()
    .any(|arg| arg == "sandbox_workspace_write.network_access=true");
  let roots: BTreeSet<String> = serde_json::from_str(
    args
      .iter()
      .find_map(|arg| arg.strip_prefix("sandbox_workspace_write.writable_roots="))
      .expect("writable roots override"),
  )
  .expect("fixture paths use JSON-compatible TOML strings");
  let extra: Vec<_> = roots.iter().filter(|root| **root != cwd.to_string_lossy()).collect();
  emit(&json!({"id":3,"result":{
    "thread":{"id":thread,"environments":[{"environmentId":"local","cwd":cwd,"runtimeWorkspaceRoots":roots}]},
    "approvalPolicy":params["approvalPolicy"],"approvalsReviewer":"user","model":params["model"],
    "sandbox":{"type":"workspaceWrite","networkAccess":network,"writableRoots":extra}
  }}));
  roots
}

fn command_request(thread: &str, turn: &str, cwd: &std::path::Path) {
  emit(&json!({"method":"item/started","params":{"item":{
    "id":"command-1","type":"commandExecution","command":"disposable-command"
  }}}));
  emit(
    &json!({"id":100,"method":"item/commandExecution/requestApproval","params":{
      "itemId":"command-1","threadId":thread,"turnId":turn,
      "command":"disposable-command","cwd":cwd,"availableDecisions":["accept","decline"]
    }}),
  );
}

fn tool_request(thread: &str, turn: &str) {
  emit(
    &json!({"method":"item/started","params":{"threadId":thread,"turnId":turn,"item":{
      "id":"mcp-1","type":"mcpToolCall","server":"disposable","tool":"write",
      "arguments":{"filePath":"mcp-once.txt"}
    }}}),
  );
  emit(&json!({"id":101,"method":"mcpServer/elicitation/request","params":{
    "threadId":thread,"turnId":turn,"serverName":"disposable","mode":"form",
    "requestedSchema":{"type":"object","properties":{}}
  }}));
}
