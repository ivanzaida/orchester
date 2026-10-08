//! The server itself: a discovery file of its own, apart from lurq's, that only the user reads and
//! that goes with the window; files of killed processes removed when it starts; no request without
//! the bearer token; every tool listed with a strict input schema.

use std::{
  fs::{self, File},
  io::{BufRead, BufReader, Write},
  net::TcpStream,
  path::Path,
};

use lurq::app::Tree;
use orchester_desktop::embedded_mcp::McpAccess;
use serde_json::{Value, json};

use super::{agent, discovered, refused};
use crate::{harness::Desktop, mcp_agent::McpAgent};

const TOOLS: [&str; 25] = [
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
  "send_message",
  "set_run_auto_mode",
  "get_auto_mode",
  "set_auto_mode",
  "remove_worktree",
  "return_to_draft",
  "continue_run",
  "list_approvals",
  "decide_approval",
  "decide_plan",
  "answer_question",
  "set_code_index",
  "update_project_limits",
  "duplicate_pipeline",
  "list_pipelines",
];

/// Pids no process of the test machine has: one left a file and ended, one still runs (its lock is
/// held by the test).
const ENDED: &str = "4000000001";
const RUNNING: &str = "4000000002";

/// The HTTP status of an `initialize` posted with `authorization`, if any.
fn status(port: u16, authorization: Option<&str>) -> String {
  status_from(port, authorization, None)
}

/// [`status`], from a web page at `origin` when given.
fn status_from(port: u16, authorization: Option<&str>, origin: Option<&str>) -> String {
  let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("the server accepts a connection");
  let body = json!({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}
  })
  .to_string();
  let authorization = authorization
    .map(|value| format!("Authorization: {value}\r\n"))
    .unwrap_or_default()
    + &origin.map(|origin| format!("Origin: {origin}\r\n")).unwrap_or_default();
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
  // An earlier process that had this pid was killed and left its file.
  fs::write(folder.join(format!("{}.json", std::process::id())), "{}").expect("this pid's old file");
  let lock = File::create(folder.join(format!("{RUNNING}.lock"))).expect("its lock");
  lock.try_lock().expect("the lock is free");
  lock
}

/// The discovery file's access list admits the user alone and inherits nothing from its folder.
#[cfg(windows)]
fn only_the_user_reads(path: &Path) {
  let output = std::process::Command::new("icacls")
    .arg(path)
    .output()
    .expect("icacls runs");
  let text = String::from_utf8_lossy(&output.stdout);
  let user = std::env::var("USERNAME").expect("USERNAME");
  let entries: Vec<&str> = text.lines().filter(|line| line.contains(":(")).collect();
  assert_eq!(entries.len(), 1, "{text}");
  assert!(entries[0].contains(&user) && entries[0].contains("(F)"), "{text}");
  assert!(!text.contains("(I)"), "nothing inherited: {text}");
}

/// The discovery file is the user's alone (`0600`).
#[cfg(unix)]
fn only_the_user_reads(path: &Path) {
  use std::os::unix::fs::PermissionsExt;
  let mode = fs::metadata(path).expect("metadata").permissions().mode();
  assert_eq!(mode & 0o777, 0o600);
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
  only_the_user_reads(&found.path);

  assert!(status(found.port, None).contains(" 401 "), "no token, no entry");
  assert!(status(found.port, Some("Bearer not-the-token")).contains(" 401 "));
  let bearer = format!("Bearer {}", found.token);
  assert!(status(found.port, Some(&bearer)).contains(" 200 "));
  let lowercase = format!("bearer {}", found.token);
  assert!(
    status(found.port, Some(&lowercase)).contains(" 200 "),
    "the scheme ignores case"
  );
  let page = Some("http://evil.example");
  assert!(
    status_from(found.port, Some(&bearer), page).contains(" 403 "),
    "no web page gets in"
  );
  let local = Some("http://localhost:5173");
  assert!(
    status_from(found.port, Some(&bearer), local).contains(" 200 "),
    "a loopback page may"
  );

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

/// lurq's server of a release build only reads: it lists and answers the tools that look at the window,
/// and refuses those that change it; a debug build's also drives it.
#[test]
fn window_mcp_of_a_release_build_only_reads() {
  let mut tree = Tree::new();
  let handle = tree.enable_mcp(McpAccess::ReadOnly.config());
  let (port, token) = (handle.port(), handle.token().to_owned());
  let mut agent = McpAgent::connect(port, token);
  let tools = agent.tools();
  let names: Vec<&str> = tools["tools"]
    .as_array()
    .expect("tools")
    .iter()
    .filter_map(|tool| tool["name"].as_str())
    .collect();
  for kept in [
    "lurq_screenshot",
    "lurq_read_tree",
    "lurq_inspect",
    "lurq_find",
    "lurq_windows",
    "lurq_wait",
  ] {
    assert!(names.contains(&kept), "{kept} in {names:?}");
  }
  for changing in [
    "lurq_interact",
    "lurq_act",
    "lurq_set_value",
    "lurq_resize",
    "lurq_navigate",
  ] {
    assert!(!names.contains(&changing), "{changing} in {names:?}");
    let reply = agent.reply(
      "tools/call",
      &json!({"name": changing, "arguments": {"action": "click", "ref": "ref_1"}}),
    );
    assert!(reply.get("error").is_some(), "{changing}: {reply}");
  }
  tree.shutdown_mcp();

  let mut tree = Tree::new();
  let handle = tree.enable_mcp(McpAccess::Drive.config());
  let mut agent = McpAgent::connect(handle.port(), handle.token().to_owned());
  let tools = agent.tools().to_string();
  assert!(tools.contains("lurq_interact"), "a debug build drives the window");
  tree.shutdown_mcp();
  assert_eq!(
    McpAccess::of_build(),
    if cfg!(debug_assertions) {
      McpAccess::Drive
    } else {
      McpAccess::ReadOnly
    }
  );
}
