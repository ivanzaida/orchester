//! A stand-in team server on loopback over plain HTTP, in the shapes of `orchester-api`, for the tests and QA builds
//! that join a team with no project open (the workspace's `startup_join`, the app's All projects and Join a team, the
//! QA app's team scenarios): what it is (`GET /server`, not linked to Orchester Cloud, so a code needs no cloud
//! identity), joining with a code (`POST /auth/code`) or a cloud identity (`POST /auth/cloud`), who signed in
//! (`GET /auth/me`), and the projects the person is a member of (`GET /projects`, by `?slug=` too). What a linked
//! checkout's first download reads is in [`first_download`]. It answers a wrong code and an expired one alike,
//! `invalid_code`, as a real server does; it can have no place for a cloud identity, and it can stop answering.

use std::{
  io::{Read, Write},
  net::{TcpListener, TcpStream},
  sync::Arc,
  thread,
};

use orchester_core::id::ProjectId;
use parking_lot::Mutex;
use serde_json::{Value, json};

mod first_download;
use first_download::TaskPages;
pub use first_download::TasksAnswer;

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
/// Who every join signs in as.
const USER: &str = "0191c7c0-0000-7000-8000-000000000b0b";

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
  /// Whether each code join carried an Orchester Cloud identity token.
  code_joins: Vec<bool>,
  cloud_joins: usize,
  /// How a project's tasks are answered.
  tasks: TasksAnswer,
  /// A project's tasks in pages, instead of its three in one answer.
  pages: Option<TaskPages>,
  /// How often a project's tasks were asked for.
  tasks_asked: usize,
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
      code_joins: Vec::new(),
      cloud_joins: 0,
      tasks: TasksAnswer::default(),
      pages: None,
      tasks_asked: 0,
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
          "role": "member", "next_number": 4,
        })
      })
      .collect();
  }

  /// The person owns the project at `slug`.
  pub fn own(&self, slug: &str) {
    let mut state = self.state.lock();
    if let Some(project) = state.projects.iter_mut().find(|project| project["slug"] == slug) {
      project["role"] = json!("owner");
    }
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

  /// Whether each code join so far carried an Orchester Cloud identity token, in order.
  pub fn code_joins(&self) -> Vec<bool> {
    self.state.lock().code_joins.clone()
  }

  pub fn cloud_joins(&self) -> usize {
    self.state.lock().cloud_joins
  }

  /// Answers a project's tasks as `tasks` from now on, held ones too.
  pub fn answer_tasks(&self, tasks: TasksAnswer) {
    self.state.lock().tasks = tasks;
  }

  /// A project has `total` tasks, read in pages: those up to number `until` are served, and what comes after them is
  /// answered as [`Self::answer_tasks`] says.
  pub fn page_tasks(&self, total: usize, until: usize) {
    self.state.lock().pages = Some(TaskPages { total, until });
  }

  /// How often a project's tasks were asked for.
  pub fn tasks_asked(&self) -> usize {
    self.state.lock().tasks_asked
  }

  /// The id here of the project at `slug`.
  pub fn project(&self, slug: &str) -> Option<ProjectId> {
    let state = self.state.lock();
    let project = state.projects.iter().find(|project| project["slug"] == slug)?;
    let id = project["id"].as_str().expect("`share` gives each project its id");
    Some(id.parse().expect("`share` makes its ids in a project id's shape"))
  }
}

fn serve(mut stream: TcpStream, state: &Mutex<State>) {
  if state.lock().down {
    return;
  }
  let Some((line, headers, body)) = read_request(&mut stream) else {
    return;
  };
  let target = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
  let (path, query) = target.split_once('?').unwrap_or((&target, ""));
  let (status, answer) = route((path, query), (&headers, &body), state);
  let answer = answer.to_string();
  let reply = format!(
    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
    answer.len()
  );
  // The test reads the answer or fails on its own; a broken connection needs nothing here.
  let _ = stream.write_all(reply.as_bytes());
}

fn route((path, query): (&str, &str), (headers, body): (&str, &str), state: &Mutex<State>) -> (&'static str, Value) {
  let request: Value = serde_json::from_str(body).unwrap_or(Value::Null);
  let signed_in = headers.contains(&format!("authorization: bearer {TOKEN}"));
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
    _ if path.starts_with("GET ") && !signed_in => ("401 Unauthorized", error("unauthorized")),
    "GET /api/v1/auth/me" => ("200 OK", json!({"user": user(false)})),
    "GET /api/v1/projects" => match query.strip_prefix("slug=") {
      Some(slug) => first_download::by_slug(slug, state),
      None => ("200 OK", json!({"projects": state.lock().projects})),
    },
    _ => first_download::answer((path, query), state).unwrap_or_else(|| ("404 Not Found", error("not_found"))),
  }
}

/// The `201` of a join `via` a code or the cloud: this computer's device token and Bohdan's account.
fn joined(via: &str) -> Value {
  json!({
    "token": TOKEN,
    "session": {"id": "0191c7c0-0000-7000-8000-00000000d0d0", "device": "Bohdan's laptop", "created_at": 1,
      "last_used_at": 1, "current": true, "via": via},
    "user": user(via == "cloud"),
    "joined": null,
  })
}

/// Bohdan's account.
fn user(cloud: bool) -> Value {
  json!({"id": USER, "username": USERNAME, "display_name": "Bohdan Koval", "admin": false, "disabled": false,
    "cloud": cloud})
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
