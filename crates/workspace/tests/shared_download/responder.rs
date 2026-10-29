//! A loopback responder for a shared project's first download (ORC-282). It is not a team server: it answers only
//! what joining with a code and reading one project need, `GET /server`, `POST /auth/code`, `GET /auth/me`, the
//! project by its slug and by its id, and the project's tags, modules and tasks, with one fixed project of three
//! tasks, one tag and one module. Its tasks can be held until the test lets them go, or refused with a `500`.
//! Anything else is `404`. Each request has a connection of its own, answered on a thread of its own.

use std::{
  io::{BufRead, BufReader, Read, Write},
  net::{TcpListener, TcpStream},
  sync::Arc,
  thread,
  time::Duration,
};

use parking_lot::{Condvar, Mutex};
use serde_json::{Value, json};

/// The project's slug, key and name, and its three tasks' titles.
pub const SLUG: &str = "kontur";
pub const KEY: &str = "KON";
pub const NAME: &str = "Kontur";
pub const TITLES: [&str; 3] = ["Sign in with a passkey", "Export the ledger", "Billing retries"];
pub const TAG: &str = "backend";
pub const MODULE: &str = "Payments";

const PROJECT: &str = "01a18f00-0000-7000-8000-000000000282";
const TAG_ID: &str = "01a18f00-0000-7000-8000-000000000a01";
const MODULE_ID: &str = "01a18f00-0000-7000-8000-000000000b01";
const USER: &str = "01a18f00-0000-7000-8000-000000000c01";
const SESSION: &str = "01a18f00-0000-7000-8000-000000000d01";
const TASK_IDS: [&str; 3] = [
  "01a18f00-0000-7000-8000-000000000e01",
  "01a18f00-0000-7000-8000-000000000e02",
  "01a18f00-0000-7000-8000-000000000e03",
];
/// The longest a held answer waits for the test, so a test that fails never leaves a client waiting for ever.
const HOLD_AT_MOST: Duration = Duration::from_secs(60);

/// How the responder answers the project's tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TasksAnswer {
  Serve,
  /// Held until the test serves or refuses them.
  Hold,
  /// `500 internal`.
  Fail,
}

struct State {
  tasks: Mutex<TasksAnswer>,
  changed: Condvar,
  /// The request lines read, e.g. `GET /api/v1/projects/…/tasks`.
  seen: Mutex<Vec<String>>,
}

pub struct Responder {
  url: String,
  state: Arc<State>,
}

impl Responder {
  /// Listens on a free loopback port and answers until the test program ends.
  pub fn start(tasks: TasksAnswer) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen on loopback");
    let url = format!("http://localhost:{}", listener.local_addr().expect("address").port());
    let state = Arc::new(State {
      tasks: Mutex::new(tasks),
      changed: Condvar::new(),
      seen: Mutex::new(Vec::new()),
    });
    let (serving, base) = (state.clone(), url.clone());
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        let (state, base) = (serving.clone(), base.clone());
        thread::spawn(move || answer(stream, &state, &base));
      }
    });
    Self { url, state }
  }

  /// The server's address, `http://localhost:<port>`.
  pub fn url(&self) -> &str {
    &self.url
  }

  /// The project's address on it.
  pub fn address(&self) -> String {
    format!("{}/{SLUG}", self.url)
  }

  pub fn project() -> orchester_core::id::ProjectId {
    PROJECT.parse().expect("a project id")
  }

  /// Answers the tasks as `tasks` from now on, held ones too.
  pub fn tasks(&self, tasks: TasksAnswer) {
    *self.state.tasks.lock() = tasks;
    self.state.changed.notify_all();
  }

  /// How often the project's tasks were asked for.
  pub fn tasks_asked(&self) -> usize {
    let tasks = format!("GET /api/v1/projects/{PROJECT}/tasks");
    self.state.seen.lock().iter().filter(|line| **line == tasks).count()
  }
}

fn answer(mut stream: TcpStream, state: &State, base: &str) {
  let Some(line) = read(&stream) else { return };
  state.seen.lock().push(line.clone());
  let (status, body) = route(&line, state, base);
  let body = body.to_string();
  let head = format!(
    "HTTP/1.1 {status} Answer\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    body.len()
  );
  // The test reads what the client made of the answer; a client that hung up fails there.
  let _ = stream.write_all(head.as_bytes());
  let _ = stream.write_all(body.as_bytes());
}

/// The request line without its version, its headers and body read; `None` for a connection that closed first.
fn read(stream: &TcpStream) -> Option<String> {
  let mut reader = BufReader::new(stream);
  let mut line = String::new();
  reader.read_line(&mut line).ok()?;
  let line = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
  let mut length = 0;
  loop {
    let mut header = String::new();
    reader.read_line(&mut header).ok()?;
    let header = header.trim_end();
    if header.is_empty() {
      break;
    }
    if let Some((name, value)) = header.split_once(':')
      && name.eq_ignore_ascii_case("content-length")
    {
      length = value.trim().parse().ok()?;
    }
  }
  let mut body = vec![0; length];
  reader.read_exact(&mut body).ok()?;
  Some(line)
}

fn route(line: &str, state: &State, base: &str) -> (u16, Value) {
  let project = format!("/api/v1/projects/{PROJECT}");
  let (method, target) = line.split_once(' ').unwrap_or((line, ""));
  let path = target.split('?').next().unwrap_or(target);
  match (method, path) {
    ("GET", "/api/v1/server") => (200, server()),
    ("POST", "/api/v1/auth/code") => (201, joined()),
    ("GET", "/api/v1/auth/me") => (200, json!({ "user": user() })),
    ("GET", "/api/v1/projects") if target.ends_with(&format!("slug={SLUG}")) => {
      (200, json!({ "project": shared_project(base) }))
    }
    ("GET", path) if path == project => (200, json!({ "project": shared_project(base) })),
    ("GET", path) if path == format!("{project}/tags") => (200, json!({ "tags": [{ "id": TAG_ID, "name": TAG }] })),
    ("GET", path) if path == format!("{project}/modules") => (200, json!({ "modules": [module()], "changes": [] })),
    ("GET", path) if path == format!("{project}/tasks") => tasks(state),
    _ => (404, json!({ "error": "not_found", "message": "Not found" })),
  }
}

fn tasks(state: &State) -> (u16, Value) {
  let mut answer = state.tasks.lock();
  state
    .changed
    .wait_while_for(&mut answer, |answer| *answer == TasksAnswer::Hold, HOLD_AT_MOST);
  match *answer {
    TasksAnswer::Serve => {
      let tasks: Vec<Value> = TASK_IDS
        .iter()
        .zip(TITLES)
        .enumerate()
        .map(|(index, (id, title))| task(id, index + 1, title))
        .collect();
      (200, json!({ "tasks": tasks, "cursor": 7 }))
    }
    TasksAnswer::Hold | TasksAnswer::Fail => (500, json!({ "error": "internal", "message": "The server failed" })),
  }
}

fn server() -> Value {
  json!({ "name": "Kontur team", "api_version": 3, "needs_setup": false, "cloud": null, "certificate": null,
    "version": "0.2.0" })
}

fn user() -> Value {
  json!({ "id": USER, "username": "bohdan", "display_name": "Bohdan Koval", "admin": true, "disabled": false,
    "cloud": false })
}

fn joined() -> Value {
  json!({
    "token": format!("orc_{}", "0".repeat(64)),
    "session": { "id": SESSION, "device": "Test computer", "created_at": 1, "last_used_at": 1, "current": true,
      "via": "code", "mfa": false },
    "user": user(),
    "joined": null,
  })
}

fn shared_project(base: &str) -> Value {
  json!({ "id": PROJECT, "slug": SLUG, "address": format!("{base}/{SLUG}"), "key": KEY, "name": NAME,
    "remote_hint": null, "role": "owner", "next_number": 4 })
}

fn attribution() -> Value {
  json!({ "by": { "id": USER, "display_name": "Bohdan Koval" }, "at": 1_000 })
}

fn module() -> Value {
  json!({ "id": MODULE_ID, "name": MODULE, "description": "", "start": null, "target": null, "state": "open",
    "revision": 1, "created": attribution(), "updated": attribution() })
}

fn task(id: &str, number: usize, title: &str) -> Value {
  json!({
    "id": id, "number": number, "key": format!("{KEY}-{number}"), "former_key": null, "revision": 1,
    "title": title, "brief": "", "criteria": [], "pipeline": null, "limits": null, "workflow": "todo",
    "priority": "high", "tags": [TAG_ID], "blocking": [], "blocked_reason": null, "module": MODULE_ID,
    "status": "draft", "has_run": false, "created": attribution(), "updated": attribution(), "claim": null,
    "run": null
  })
}
