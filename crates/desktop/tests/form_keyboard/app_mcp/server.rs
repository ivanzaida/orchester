//! The server itself: a discovery file of its own, apart from lurq's, that only the user reads and
//! that goes with the window; files of killed processes removed when it starts; no request without
//! the bearer token; every tool listed with a strict input schema.

use std::{
  fs::{self, File},
  io::{BufRead, BufReader, Write},
  net::TcpStream,
  path::Path,
};

use serde_json::{Value, json};

use super::{agent, discovered, refused};
use crate::harness::Desktop;

const TOOLS: [&str; 13] = [
  "list_projects",
  "list_tasks",
  "get_task",
  "create_task",
  "update_task",
  "run_task",
  "get_run",
  "pause_run",
  "resume_run",
  "cancel_run",
  "list_approvals",
  "decide_approval",
  "list_pipelines",
];

/// Pids no process of the test machine has: one left a file and ended, one still runs (its lock is
/// held by the test).
const ENDED: &str = "4000000001";
const RUNNING: &str = "4000000002";

/// The HTTP status of an `initialize` posted with `authorization`, if any.
fn status(port: u16, authorization: Option<&str>) -> String {
  let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("the server accepts a connection");
  let body = json!({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}
  })
  .to_string();
  let authorization = authorization
    .map(|value| format!("Authorization: {value}\r\n"))
    .unwrap_or_default();
  write!(
    stream,
    "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{authorization}Content-Type: application/json\r\n\
     Accept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
    body.len()
  )
  .expect("the request is sent");
  let mut line = String::new();
  BufReader::new(stream).read_line(&mut line).expect("a status line");
  line
}

/// Leaves the files of an ended process and of a running one in `folder`; the running one's lock is
/// held while the returned file is.
fn others(folder: &Path) -> File {
  fs::create_dir_all(folder).expect("the discovery folder");
  fs::write(folder.join(format!("{ENDED}.json")), "{}").expect("an ended process's file");
  fs::write(folder.join(format!("{RUNNING}.json")), "{}").expect("a running process's file");
  let lock = File::create(folder.join(format!("{RUNNING}.lock"))).expect("its lock");
  lock.try_lock().expect("the lock is free");
  lock
}

#[test]
fn app_mcp_writes_its_own_discovery_file_and_requires_the_token() {
  let mut running = None;
  let desktop = Desktop::with_seeded_project_and_app_mcp(|repo| {
    let folder = repo.parent().expect("the test folder").join("orchester-mcp");
    running = Some(others(&folder));
  });
  let folder = desktop.mcp_folder();
  assert!(
    !folder.join(format!("{ENDED}.json")).exists(),
    "an ended process's file goes"
  );
  assert!(
    folder.join(format!("{RUNNING}.json")).exists(),
    "a running process's file stays"
  );

  let found = discovered(&desktop);
  assert_eq!(found.contents["app"], "orchester");
  assert_eq!(found.contents["pid"], std::process::id());
  assert_eq!(found.contents["transport"], "streamable-http");
  assert_eq!(found.contents["url"], format!("http://127.0.0.1:{}/mcp", found.port));
  assert!(found.token.len() >= 32, "a long random token");

  assert!(status(found.port, None).contains(" 401 "), "no token, no entry");
  assert!(status(found.port, Some("Bearer not-the-token")).contains(" 401 "));
  let bearer = format!("Bearer {}", found.token);
  assert!(status(found.port, Some(&bearer)).contains(" 200 "));

  let mut agent = agent(&desktop);
  let tools = agent.tools();
  let names: Vec<&str> = tools["tools"]
    .as_array()
    .expect("tools")
    .iter()
    .filter_map(|tool| tool["name"].as_str())
    .collect();
  assert_eq!(names, TOOLS);
  for tool in tools["tools"].as_array().expect("tools") {
    assert_eq!(tool["inputSchema"]["additionalProperties"], false, "{tool}");
  }
  let error = refused(&mut agent, "list_projects", &json!({"verbose": true}));
  assert_eq!(error["code"], "invalid_arguments");
  let error = refused(
    &mut agent,
    "list_tasks",
    &json!({"project": "Kontur", "status": "almost_done"}),
  );
  assert_eq!(error["code"], "invalid_arguments");

  let path = found.path;
  drop(desktop);
  assert!(!path.exists(), "the discovery file goes with the window");
  drop(running);
}

#[test]
fn app_mcp_lists_the_projects_of_the_workspace() {
  let desktop = Desktop::with_seeded_project_and_app_mcp(|_| {});
  let mut agent = agent(&desktop);
  let listed = super::ok(&mut agent, "list_projects", &json!({}));
  let projects = listed["projects"].as_array().expect("projects");
  assert_eq!(projects.len(), 1);
  assert_eq!(projects[0]["name"], "Kontur");
  assert_eq!(projects[0]["folder_missing"], false);
  let id = projects[0]["id"].as_str().expect("an id");
  // Named by id or, ignoring case, by name.
  for named in [id, "kontur"] {
    let tasks = super::ok(&mut agent, "list_tasks", &json!({ "project": named }));
    assert_eq!(tasks["tasks"], Value::Array(Vec::new()));
  }
  let error = refused(&mut agent, "list_tasks", &json!({"project": "Atlas"}));
  assert_eq!(error["code"], "project_not_found");
}
