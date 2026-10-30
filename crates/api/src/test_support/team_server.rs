//! A stand-in team server on loopback over plain HTTP, in the shapes of `orchester-api`, for the tests and QA builds
//! that join a team with no project open (the workspace's `startup_join`, the app's All projects and Join a team):
//! what it is (`GET /server`, not linked to Orchester Cloud, so a code needs no cloud identity), joining with a code
//! (`POST /auth/code`) or a cloud identity (`POST /auth/cloud`), the projects the person is a member of
//! (`GET /projects`), and what a first link of a project with no tasks reads and sends (the workspace's `local_owner`).
//! It answers a wrong code and an expired one alike, `invalid_code`, as a real server does; it can have no place for a
//! cloud identity, it can refuse a link, and it can stop answering.

use std::{
  io::{Read, Write},
  net::{TcpListener, TcpStream},
  sync::Arc,
  thread,
};

use parking_lot::Mutex;
use serde_json::{Value, json};

/// The server's name, as it gives it at `GET /server`, unless it is [`TeamServer::named`] otherwise.
pub const NAME: &str = "Kontur team";
/// The invite code that joins.
pub const CODE: &str = "K7QM3XRD";
/// A code the server made once; it expires when the test says so.
pub const LATER_CODE: &str = "Z9XW4TPL";
/// The username the joins make.
pub const USERNAME: &str = "bohdan";
/// The device token every join issues.
const TOKEN: &str = "orc_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct State {
  /// The name it gives at `GET /server`.
  name: String,
  /// The projects `GET /projects` lists.
  projects: Vec<Value>,
  /// `POST /auth/cloud` answers `not_invited`.
  not_invited: bool,
  /// [`LATER_CODE`] has expired.
  later_expired: bool,
  /// Connections are closed without an answer, as when the server cannot be reached.
  down: bool,
  /// `POST /projects/{id}/link` answers `forbidden`.
  refuses_links: bool,
  /// What `GET /projects/{id}/tasks` lists, of every project.
  tasks: Vec<Value>,
  /// Whether each code join carried an Orchester Cloud identity token.
  code_joins: Vec<bool>,
  cloud_joins: usize,
}

pub struct TeamServer {
  port: u16,
  state: Arc<Mutex<State>>,
}

impl TeamServer {
  /// The server named [`NAME`].
  pub fn start() -> Self {
    Self::named(NAME)
  }

  /// The server named `name`, e.g. *Acme* as the design names the team.
  pub fn named(name: &str) -> Self {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("listen");
    let port = listener.local_addr().expect("address").port();
    let state = Arc::new(Mutex::new(State {
      name: name.to_owned(),
      projects: Vec::new(),
      not_invited: false,
      later_expired: false,
      down: false,
      refuses_links: false,
      tasks: Vec::new(),
      code_joins: Vec::new(),
      cloud_joins: 0,
    }));
    let serving = state.clone();
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        let state = serving.clone();
        thread::spawn(move || serve(stream, &state));
      }
    });
    Self { port, state }
  }

  /// Its address, as the person types it and Orchester Cloud names it.
  pub fn url(&self) -> String {
    format!("http://127.0.0.1:{}", self.port)
  }

  /// The person's projects there: `(name, slug, remote hint)`.
  pub fn share(&self, projects: &[(&str, &str, Option<&str>)]) {
    let url = self.url();
    self.state.lock().projects = projects
      .iter()
      .enumerate()
      .map(|(index, (name, slug, hint))| {
        json!({
          "id": format!("0191c7c0-0000-7000-8000-00000000{index:04x}"), "slug": slug,
          "address": format!("{url}/{slug}"), "key": "KON", "name": name, "remote_hint": hint,
          "role": "member", "next_number": 1,
        })
      })
      .collect();
  }

  pub fn not_invited(&self) {
    self.state.lock().not_invited = true;
  }

  pub fn expire_later_code(&self) {
    self.state.lock().later_expired = true;
  }

  pub fn set_down(&self, down: bool) {
    self.state.lock().down = down;
  }

  /// Every project holds one task titled `title` from now on, so a checkout with tasks of its own joins through its
  /// review.
  pub fn hold_task(&self, title: &str) {
    let person = json!({"by": {"id": "0191c7c0-0000-7000-8000-000000000b0b", "display_name": "Bohdan Koval"}, "at": 1});
    self.state.lock().tasks = vec![json!({
      "id": "0191c7c0-0000-7000-8000-00000000a001", "number": 1, "key": "KON-1", "former_key": null, "revision": 1,
      "title": title, "brief": "", "criteria": [], "pipeline": null, "limits": null, "workflow": "backlog",
      "priority": "none", "tags": [], "blocking": [], "blocked_reason": null, "module": null, "status": "queued",
      "has_run": false, "created": person, "updated": person, "claim": null, "run": null,
    })];
  }

  /// A first link of a project is refused from now on, as a server that took the person's role away refuses it.
  pub fn refuse_links(&self) {
    self.state.lock().refuses_links = true;
  }

  /// Whether each code join so far carried an Orchester Cloud identity token, in order.
  pub fn code_joins(&self) -> Vec<bool> {
    self.state.lock().code_joins.clone()
  }

  pub fn cloud_joins(&self) -> usize {
    self.state.lock().cloud_joins
  }
}

fn serve(mut stream: TcpStream, state: &Mutex<State>) {
  if state.lock().down {
    return;
  }
  let Some((line, headers, body)) = read_request(&mut stream) else {
    return;
  };
  let path = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
  let (status, answer) = route(&path, (&headers, &body), state);
  let answer = answer.to_string();
  let reply = format!(
    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
    answer.len()
  );
  // The test reads the answer or fails on its own; a broken connection needs nothing here.
  let _ = stream.write_all(reply.as_bytes());
}

fn route(path: &str, (headers, body): (&str, &str), state: &Mutex<State>) -> (&'static str, Value) {
  let request: Value = serde_json::from_str(body).unwrap_or(Value::Null);
  match path {
    "GET /api/v1/server" => {
      let name = state.lock().name.clone();
      (
        "200 OK",
        json!({"name": name, "api_version": 3, "needs_setup": false, "cloud": null, "version": "0.4.0"}),
      )
    }
    "POST /api/v1/auth/code" => {
      let mut state = state.lock();
      state.code_joins.push(request.get("identity_token").is_some());
      let code = request["code"].as_str().unwrap_or_default();
      if code == CODE || (code == LATER_CODE && !state.later_expired) {
        ("201 Created", joined("code"))
      } else {
        ("400 Bad Request", error("invalid_code"))
      }
    }
    "POST /api/v1/auth/cloud" => {
      let mut state = state.lock();
      state.cloud_joins += 1;
      if state.not_invited {
        ("403 Forbidden", error("not_invited"))
      } else {
        ("201 Created", joined("cloud"))
      }
    }
    path if path.starts_with("GET /api/v1/projects") || path.starts_with("POST /api/v1/projects") => {
      if headers.contains(&format!("authorization: bearer {TOKEN}")) {
        project_route(path, &state.lock())
      } else {
        ("401 Unauthorized", error("unauthorized"))
      }
    }
    _ => ("404 Not Found", error("not_found")),
  }
}

/// What a project's endpoints answer: the projects, and for one of them what a first link reads and sends. It holds no
/// task, so a first link of a project with no local tasks goes through.
fn project_route(path: &str, state: &State) -> (&'static str, Value) {
  let (verb, rest) = path.split_once(" /api/v1/projects").unwrap_or((path, ""));
  if verb == "GET" && rest.is_empty() {
    return ("200 OK", json!({"projects": state.projects}));
  }
  let found = |wanted: &str, field: &str| state.projects.iter().find(|project| project[field] == wanted).cloned();
  if let Some(slug) = rest.strip_prefix("?slug=") {
    return found(slug, "slug").map_or_else(
      || ("404 Not Found", error("not_found")),
      |project| ("200 OK", json!({"project": project})),
    );
  }
  let rest = rest.trim_start_matches('/');
  let (id, part) = rest.split_once('/').unwrap_or((rest, ""));
  let Some(project) = found(id, "id") else {
    return ("404 Not Found", error("not_found"));
  };
  match (verb, part.split('?').next().unwrap_or_default()) {
    ("GET", "") => ("200 OK", json!({"project": project})),
    ("GET", "tasks") => ("200 OK", json!({"tasks": state.tasks, "cursor": 0})),
    ("GET", "tags") => ("200 OK", json!({"tags": []})),
    ("GET", "modules") => ("200 OK", json!({"modules": [], "changes": []})),
    ("POST", "link") if state.refuses_links => ("403 Forbidden", error("forbidden")),
    ("POST", "link") => ("200 OK", json!({"tasks": [], "dropped_links": [], "cursor": 0})),
    _ => ("404 Not Found", error("not_found")),
  }
}

/// The `201` of a join `via` a code or the cloud: this computer's device token and Bohdan's account.
fn joined(via: &str) -> Value {
  json!({
    "token": TOKEN,
    "session": {"id": "0191c7c0-0000-7000-8000-00000000d0d0", "device": "Bohdan's laptop", "created_at": 1,
      "last_used_at": 1, "current": true, "via": via},
    "user": {"id": "0191c7c0-0000-7000-8000-000000000b0b", "username": USERNAME, "display_name": "Bohdan Koval",
      "admin": false, "disabled": false, "cloud": via == "cloud"},
    "joined": null,
  })
}

fn error(code: &str) -> Value {
  json!({"error": code, "message": code})
}

/// The request line, the headers in lowercase, and the body.
fn read_request(stream: &mut TcpStream) -> Option<(String, String, String)> {
  let mut buffer = Vec::new();
  let mut chunk = [0_u8; 4096];
  loop {
    let read = stream.read(&mut chunk).ok()?;
    if read == 0 {
      return None;
    }
    buffer.extend_from_slice(&chunk[..read]);
    let text = String::from_utf8_lossy(&buffer).into_owned();
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
      continue;
    };
    let headers = head.to_ascii_lowercase();
    let length = headers
      .lines()
      .find_map(|line| line.strip_prefix("content-length:"))
      .and_then(|value| value.trim().parse::<usize>().ok())
      .unwrap_or(0);
    if body.len() >= length {
      let line = head.lines().next().unwrap_or_default().to_owned();
      return Some((line, headers, body.to_owned()));
    }
  }
}
