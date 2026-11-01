//! A loopback responder for a shared project's first download (ORC-282, ORC-291). It is not a team server: it answers
//! only what joining with a code and reading one project need, `GET /server`, `POST /auth/code`, `GET /auth/me`, the
//! project by its slug and by its id, and the project's tags, modules and tasks, with one fixed project of one tag and
//! one module. Its three fixed tasks ignore `after`, as a server without pages does; they can be held until the test
//! lets them go, or refused with a `500`. A paged responder ([`Responder::paged`]) has as many tasks as the test says
//! instead, read in pages each with an answer of its own, and the whole list unheld. The project can leave the server.
//! Anything else is `404`. Each request has a connection of its own, answered on a thread of its own.

use std::{
  collections::HashMap,
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

/// How a paged responder answers the page after one number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageAnswer {
  Serve,
  /// Held until the test says otherwise.
  Hold,
  /// `500 internal`.
  Fail,
  /// The connection breaks without an answer.
  Drop,
  /// A page whose `next` is where it started.
  Stuck,
  /// A page without tasks, whose `next` is one further.
  Empty,
  /// The tasks up to where it starts again, with a `next` one further.
  Repeats,
  /// Every task as one page, as a server that does not know `after`.
  Whole,
  /// [`Self::Whole`], highest number first.
  NewestFirst,
}

/// A paged responder's tasks and how each page is answered.
struct Paging {
  tasks: usize,
  per_page: usize,
  total: Option<usize>,
  answers: HashMap<u32, PageAnswer>,
  /// The number of the task that blocks a task, by that task's number.
  blockers: HashMap<usize, usize>,
}

impl Paging {
  fn answer(&self, after: u32) -> PageAnswer {
    self.answers.get(&after).copied().unwrap_or(PageAnswer::Serve)
  }

  fn numbered(&self, numbers: std::ops::RangeInclusive<usize>) -> Vec<Value> {
    numbers
      .map(|number| {
        let mut task = task(&numbered_id(number), number, &format!("Task {number}"));
        if let Some(blocker) = self.blockers.get(&number) {
          task["blocking"] = json!([{ "task": numbered_id(*blocker), "gate": "merge" }]);
        }
        task
      })
      .collect()
  }
}

struct State {
  tasks: Mutex<TasksAnswer>,
  changed: Condvar,
  paging: Mutex<Option<Paging>>,
  page_changed: Condvar,
  project_gone: Mutex<bool>,
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
    Self::listen(tasks, None)
  }

  /// A responder whose project has `tasks` tasks, read `per_page` at a time, each page naming `total`.
  pub fn paged(tasks: usize, per_page: usize, total: Option<usize>) -> Self {
    let paging = Paging {
      tasks,
      per_page,
      total,
      answers: HashMap::new(),
      blockers: HashMap::new(),
    };
    Self::listen(TasksAnswer::Serve, Some(paging))
  }

  fn listen(tasks: TasksAnswer, paging: Option<Paging>) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen on loopback");
    let url = format!("http://localhost:{}", listener.local_addr().expect("address").port());
    let state = Arc::new(State {
      tasks: Mutex::new(tasks),
      changed: Condvar::new(),
      paging: Mutex::new(paging),
      page_changed: Condvar::new(),
      project_gone: Mutex::new(false),
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

  /// Answers the page after `after` as `answer` from now on, a held one too.
  pub fn page(&self, after: u32, answer: PageAnswer) {
    let mut paging = self.state.paging.lock();
    paging
      .as_mut()
      .expect("a paged responder")
      .answers
      .insert(after, answer);
    self.state.page_changed.notify_all();
  }

  /// The task numbered `task` is blocked by the one numbered `blocker`, until that one's merge.
  pub fn blocked_by(&self, task: usize, blocker: usize) {
    let mut paging = self.state.paging.lock();
    paging
      .as_mut()
      .expect("a paged responder")
      .blockers
      .insert(task, blocker);
  }

  /// From now on the project answers `404`, as for a person no longer a member.
  pub fn project_gone(&self) {
    *self.state.project_gone.lock() = true;
  }

  /// How often the project's tasks were asked for, whole or a page.
  pub fn tasks_asked(&self) -> usize {
    let tasks = format!("GET /api/v1/projects/{PROJECT}/tasks");
    let seen = self.state.seen.lock();
    seen
      .iter()
      .filter(|line| line.split('?').next() == Some(tasks.as_str()))
      .count()
  }

  /// Where each page asked for started, in the order they were asked for.
  pub fn pages_asked(&self) -> Vec<u32> {
    let tasks = format!("GET /api/v1/projects/{PROJECT}/tasks");
    let seen = self.state.seen.lock();
    seen
      .iter()
      .filter(|line| line.split('?').next() == Some(tasks.as_str()))
      .filter_map(|line| after_of(line))
      .collect()
  }
}

fn answer(mut stream: TcpStream, state: &State, base: &str) {
  let Some(line) = read(&stream) else { return };
  state.seen.lock().push(line.clone());
  let Some((status, body)) = route(&line, state, base) else {
    return;
  };
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

/// What `line` is answered with; `None` to break the connection instead.
fn route(line: &str, state: &State, base: &str) -> Option<(u16, Value)> {
  let project = format!("/api/v1/projects/{PROJECT}");
  let (method, target) = line.split_once(' ').unwrap_or((line, ""));
  let path = target.split('?').next().unwrap_or(target);
  if method == "GET" && path == format!("{project}/tasks") {
    return tasks(target, state);
  }
  Some(match (method, path) {
    ("GET", "/api/v1/server") => (200, server()),
    ("POST", "/api/v1/auth/code") => (201, joined()),
    ("GET", "/api/v1/auth/me") => (200, json!({ "user": user() })),
    ("GET", "/api/v1/projects") if target.ends_with(&format!("slug={SLUG}")) => {
      (200, json!({ "project": shared_project(base) }))
    }
    ("GET", path) if path == project && !*state.project_gone.lock() => {
      (200, json!({ "project": shared_project(base) }))
    }
    ("GET", path) if path == format!("{project}/tags") => (200, json!({ "tags": [{ "id": TAG_ID, "name": TAG }] })),
    ("GET", path) if path == format!("{project}/modules") => (200, json!({ "modules": [module()], "changes": [] })),
    _ => (404, json!({ "error": "not_found", "message": "Not found" })),
  })
}

/// The project's tasks as `target` asks: in pages when the responder has pages and it names where one starts.
fn tasks(target: &str, state: &State) -> Option<(u16, Value)> {
  let paged = state.paging.lock().is_some();
  match (paged, after_of(target)) {
    (true, Some(after)) => page(state, after),
    (true, None) => {
      let paging = state.paging.lock();
      let all = paging
        .as_ref()
        .map_or_else(Vec::new, |paging| paging.numbered(1..=paging.tasks));
      Some((200, json!({ "tasks": all, "cursor": 7 })))
    }
    (false, _) => Some(whole_list(state)),
  }
}

fn after_of(target: &str) -> Option<u32> {
  let (_, query) = target.split_once('?')?;
  query.strip_prefix("after=")?.parse().ok()
}

fn whole_list(state: &State) -> (u16, Value) {
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
    TasksAnswer::Hold | TasksAnswer::Fail => refused(),
  }
}

/// The page of the tasks numbered above `after`, as the test said to answer it.
fn page(state: &State, after: u32) -> Option<(u16, Value)> {
  let mut paging = state.paging.lock();
  state.page_changed.wait_while_for(
    &mut paging,
    |paging| {
      paging
        .as_ref()
        .is_some_and(|paging| paging.answer(after) == PageAnswer::Hold)
    },
    HOLD_AT_MOST,
  );
  let paging = paging.as_ref()?;
  let first = after as usize + 1;
  let last = (after as usize + paging.per_page).min(paging.tasks);
  let mut body = match paging.answer(after) {
    PageAnswer::Serve => json!({ "tasks": paging.numbered(first..=last), "cursor": 7 }),
    PageAnswer::Whole => return Some((200, json!({ "tasks": paging.numbered(1..=paging.tasks), "cursor": 7 }))),
    PageAnswer::NewestFirst => {
      let mut tasks = paging.numbered(1..=paging.tasks);
      tasks.reverse();
      return Some((200, json!({ "tasks": tasks, "cursor": 7 })));
    }
    PageAnswer::Stuck => json!({ "tasks": paging.numbered(first..=last), "cursor": 7, "next": after }),
    PageAnswer::Empty => json!({ "tasks": [], "cursor": 7, "next": after + 1 }),
    PageAnswer::Repeats => json!({ "tasks": paging.numbered(1..=after as usize), "cursor": 7, "next": after + 1 }),
    PageAnswer::Hold | PageAnswer::Fail => return Some(refused()),
    PageAnswer::Drop => return None,
  };
  if paging.answer(after) == PageAnswer::Serve && last < paging.tasks {
    body["next"] = json!(last);
  }
  if let Some(total) = paging.total {
    body["total"] = json!(total);
  }
  Some((200, body))
}

fn numbered_id(number: usize) -> String {
  format!("01a18f00-0000-7000-8000-{:012x}", 0xe00 + number)
}

fn refused() -> (u16, Value) {
  (500, json!({ "error": "internal", "message": "The server failed" }))
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
