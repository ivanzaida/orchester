//! A fake team server on loopback for the S10 reads and the S11 subscription: one project with its tasks and modules,
//! the members' tokens, and a log of committed batches, each with the scopes it invalidates. It filters, sorts, counts
//! and pages the task list over opaque page cursors bound to a snapshot ([`listing`]) and streams the subscription
//! ([`subscription`]). A test changes tasks, holds task reads in flight, drops the streams, resets the next one,
//! expires page snapshots and revokes a member.

mod listing;
mod subscription;

use std::{
  net::{TcpListener, TcpStream},
  sync::{Arc, Condvar, Mutex, MutexGuard},
  thread,
  time::{Duration, Instant},
};

use orchester_core::id::{ModuleId, ProjectId, TaskId};
use serde_json::{Value, json};

use crate::http::{Request, failure, respond};

/// The cursor before the first batch: the project had a history before the test.
const BEFORE_FIRST: u64 = 99;

#[derive(Clone)]
pub struct TeamServer {
  pub url: String,
  pub project: ProjectId,
  shared: Arc<Shared>,
}

struct Shared {
  state: Mutex<State>,
  /// Wakes the streams and the held reads on every change.
  changed: Condvar,
}

/// A task as the fake keeps it; its read is made from it ([`listing::task_read`]).
#[derive(Clone)]
pub struct Task {
  pub id: TaskId,
  pub number: u32,
  pub title: String,
  pub workflow: &'static str,
  pub priority: &'static str,
  pub module: Option<ModuleId>,
  pub revision: u64,
}

struct State {
  members: Vec<String>,
  revoked: Vec<String>,
  tasks: Vec<Task>,
  module: ModuleId,
  /// Each committed batch: its cursor and the scopes it invalidates.
  batches: Vec<(u64, Vec<Value>)>,
  /// Page snapshots at or below this cursor are gone.
  expired_through: u64,
  /// Streams opened before this generation end.
  streams: u64,
  reset_next: bool,
  /// The `Last-Event-ID` of each subscription opened.
  subscribed: Vec<Option<String>>,
  /// While set, a task read is answered with what it read only once released.
  hold: bool,
  held: usize,
  task_reads: usize,
}

impl State {
  fn cursor(&self) -> u64 {
    self.batches.last().map_or(BEFORE_FIRST, |(cursor, _)| *cursor)
  }

  fn task(&mut self, id: TaskId) -> &mut Task {
    self
      .tasks
      .iter_mut()
      .find(|task| task.id == id)
      .expect("a task of the project")
  }

  fn commit(&mut self, scopes: Vec<Value>) {
    let cursor = self.cursor() + 1;
    self.batches.push((cursor, scopes));
  }
}

impl TeamServer {
  /// A server whose members are `members` (tokens) and whose project has `tasks` (title, workflow, priority), the
  /// first half of them in its one module.
  pub fn start(members: &[&str], tasks: &[(&str, &'static str, &'static str)]) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    // By address: `localhost` tries IPv6 first, which costs every connection a few hundred milliseconds on Windows.
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let module = ModuleId::generate();
    let half = tasks.len() / 2;
    let tasks = tasks
      .iter()
      .zip(1..)
      .map(|(&(title, workflow, priority), number)| Task {
        id: TaskId::generate(),
        number,
        title: title.to_owned(),
        workflow,
        priority,
        module: (number as usize <= half).then_some(module),
        revision: 1,
      })
      .collect();
    let shared = Arc::new(Shared {
      state: Mutex::new(State {
        members: members.iter().map(|token| (*token).to_owned()).collect(),
        revoked: Vec::new(),
        tasks,
        module,
        batches: Vec::new(),
        expired_through: 0,
        streams: 0,
        reset_next: false,
        subscribed: Vec::new(),
        hold: false,
        held: 0,
        task_reads: 0,
      }),
      changed: Condvar::new(),
    });
    let project = ProjectId::generate();
    let serving = shared.clone();
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        let shared = serving.clone();
        thread::spawn(move || answer(&shared, project, stream));
      }
    });
    Self { url, project, shared }
  }

  fn state(&self) -> MutexGuard<'_, State> {
    self.shared.state.lock().expect("state")
  }

  fn changed(&self) {
    self.shared.changed.notify_all();
  }

  pub fn tasks(&self) -> Vec<Task> {
    self.state().tasks.clone()
  }

  pub fn module(&self) -> ModuleId {
    self.state().module
  }

  /// The cursor of the last committed batch.
  pub fn cursor(&self) -> u64 {
    self.state().cursor()
  }

  /// Retitles `task` in one committed batch that invalidates the task and the list.
  pub fn retitle(&self, task: TaskId, title: &str) {
    let mut state = self.state();
    let changed = state.task(task);
    title.clone_into(&mut changed.title);
    changed.revision += 1;
    state.commit(vec![
      json!({"kind": "task_list"}),
      json!({"kind": "task", "task": task}),
    ]);
    drop(state);
    self.changed();
  }

  /// Holds every task read from now on in flight, answering it with what it read once [`Self::release`] is called.
  pub fn hold(&self) {
    self.state().hold = true;
  }

  pub fn release(&self) {
    self.state().hold = false;
    self.changed();
  }

  /// Waits until `count` task reads are held.
  pub fn until_held(&self, count: usize) {
    self.until(|state| state.held >= count);
  }

  /// How many task reads were answered or are held.
  pub fn task_reads(&self) -> usize {
    self.state().task_reads
  }

  /// Ends every open stream, as a restart of the server would.
  pub fn drop_streams(&self) {
    self.state().streams += 1;
    self.changed();
  }

  /// The next subscription gets `reset` at the current cursor: its resume point is below the replay floor.
  pub fn reset_next(&self) {
    self.state().reset_next = true;
  }

  /// The `Last-Event-ID` of each subscription opened so far.
  pub fn subscribed(&self) -> Vec<Option<String>> {
    self.state().subscribed.clone()
  }

  /// Waits until `count` subscriptions were opened.
  pub fn until_subscribed(&self, count: usize) {
    self.until(|state| state.subscribed.len() >= count);
  }

  /// Drops every page snapshot taken so far.
  pub fn expire_pages(&self) {
    let mut state = self.state();
    state.expired_through = state.cursor();
  }

  /// Ends `token`'s membership: its streams get `revoked`, its requests `404 not_found`.
  pub fn revoke(&self, token: &str) {
    self.state().revoked.push(token.to_owned());
    self.changed();
  }

  fn until(&self, done: impl Fn(&State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut state = self.state();
    while !done(&state) {
      assert!(Instant::now() < deadline, "the server did not get there in time");
      state = self
        .shared
        .changed
        .wait_timeout(state, Duration::from_millis(20))
        .expect("state")
        .0;
    }
  }
}

/// Whose token a request carries: a member, a member no longer, or nobody known.
enum Caller {
  Member(String),
  Revoked,
  Unknown,
}

fn caller(state: &State, request: &Request) -> Caller {
  let token = request.token().unwrap_or_default();
  if state.revoked.iter().any(|revoked| revoked == token) {
    return Caller::Revoked;
  }
  if state.members.iter().any(|member| member == token) {
    return Caller::Member(token.to_owned());
  }
  Caller::Unknown
}

/// One request on the team server's `/api/v2` routes. As there, a token it does not accept is `401 unauthorized` and
/// anyone who may not read the workspace, a removed member too, `404 not_found`, both in the `/api/v1` form.
fn answer(shared: &Shared, project: ProjectId, mut stream: TcpStream) {
  let Some(request) = Request::read(&stream) else {
    return;
  };
  let prefix = format!("/api/v2/projects/{project}");
  let token = match caller(&shared.state.lock().expect("state"), &request) {
    Caller::Member(token) => token,
    Caller::Revoked => return failure(&mut stream, 404, "not_found"),
    Caller::Unknown => return failure(&mut stream, 401, "unauthorized"),
  };
  let Some(route) = request.path.strip_prefix(&prefix) else {
    return failure(&mut stream, 404, "not_found");
  };
  let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
  match (request.method.as_str(), segments.as_slice()) {
    ("GET", ["subscription"]) => subscription::stream(shared, &request, &token, &mut stream),
    ("POST", ["task-list"]) => listing::page(shared, project, &request, &mut stream),
    ("POST", ["task-counts"]) => listing::counts(shared, &request, &mut stream),
    ("GET", ["tasks", task]) => task_read(shared, project, task.parse().expect("a task id"), &mut stream),
    ("POST", ["module-list"]) => listing::modules(shared, project, &request, &mut stream),
    ("GET", ["modules", module, "progress"]) => {
      listing::progress(shared, module.parse().expect("a module id"), &mut stream);
    }
    _ => failure(&mut stream, 404, "not_found"),
  }
}

/// `GET …/tasks/{task}`: the task as it is when the request arrives, answered at once or, while reads are held, once
/// released.
fn task_read(shared: &Shared, project: ProjectId, id: TaskId, stream: &mut TcpStream) {
  let mut state = shared.state.lock().expect("state");
  state.task_reads += 1;
  let read = listing::task_read(project, state.task(id));
  if state.hold {
    state.held += 1;
    shared.changed.notify_all();
    let state = shared.changed.wait_while(state, |state| state.hold).expect("state");
    drop(state);
  } else {
    drop(state);
  }
  respond(stream, 200, &read);
}
