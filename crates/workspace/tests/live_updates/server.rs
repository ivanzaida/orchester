//! A fake team server on loopback for the live updates: one project with its tasks, the members' tokens, and an event
//! log its streams follow. It serves the snapshot to a stream without `Last-Event-ID` and the deltas after the cursor
//! to one with it, live as they are appended; reads one task; and takes a task change, a claim and a run report, each
//! appending its delta. A member's session can be revoked, and streams can be dropped or refused for a while.

use std::{
  io::{BufRead, BufReader, Read, Write},
  net::{TcpListener, TcpStream},
  sync::{Arc, Condvar, Mutex, MutexGuard},
  thread,
  time::Duration,
};

use orchester_api::{
  Attribution, ClaimAction, ClaimTake, Delta, Event, RunReport, RunStatus, RunStep, SharedClaim, SharedProject,
  SharedTag, SharedTask, SnapshotBegin, SnapshotEnd, SnapshotTasks, TaskChanged, TaskClaim, TaskContent, TaskPatch,
  TaskRun, UserRef,
};
use orchester_core::{
  id::{ClaimId, ProjectId, TagId, TaskId, UserId},
  sharing::{MemberRole, TaskRevision},
  task::{
    TaskStatus,
    organization::{ChangeActor, Priority, WorkflowStatus},
  },
};
use serde_json::{Value, json};

/// The first event's sequence: the project had a history before the test.
const FIRST_EVENT: u64 = 100;
const KEEP_ALIVE: Duration = Duration::from_millis(200);

pub struct TeamServer {
  pub url: String,
  pub project: ProjectId,
  shared: Arc<Shared>,
}

struct Shared {
  state: Mutex<State>,
  appended: Condvar,
}

struct State {
  members: Vec<(String, UserRef)>,
  tasks: Vec<SharedTask>,
  tags: Vec<SharedTag>,
  /// Each delta's sequence and its SSE text.
  events: Vec<(u64, String)>,
  /// The `Last-Event-ID` of each stream opened, in order.
  connects: Vec<Option<u64>>,
  reads: Vec<TaskId>,
  revoked: Vec<String>,
  /// Streams opened before this generation end.
  generation: u64,
  refuse_streams: usize,
  clock: i64,
}

impl TeamServer {
  /// A server whose project has `tasks` titled so, and whose members are `members` (token, name).
  pub fn start(members: &[(&str, &str)], titles: &[&str]) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    // By address: `localhost` tries IPv6 first, which costs every connection a few hundred milliseconds on Windows.
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let members: Vec<_> = members
      .iter()
      .map(|(token, name)| {
        let user = UserRef {
          id: UserId::generate(),
          display_name: (*name).to_owned(),
        };
        ((*token).to_owned(), user)
      })
      .collect();
    let creator = members[0].1.clone();
    let tasks = titles
      .iter()
      .zip(1..)
      .map(|(title, number)| task(number, title, &creator))
      .collect();
    let shared = Arc::new(Shared {
      state: Mutex::new(State {
        members,
        tasks,
        tags: vec![SharedTag {
          id: TagId::generate(),
          name: "backend".to_owned(),
        }],
        events: Vec::new(),
        connects: Vec::new(),
        reads: Vec::new(),
        revoked: Vec::new(),
        generation: 0,
        refuse_streams: 0,
        clock: 1_000,
      }),
      appended: Condvar::new(),
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

  pub fn task_ids(&self) -> Vec<TaskId> {
    self.state().tasks.iter().map(|task| task.id).collect()
  }

  pub fn tag_names(&self) -> Vec<String> {
    self.state().tags.iter().map(|tag| tag.name.clone()).collect()
  }

  /// The `Last-Event-ID` of each stream opened so far.
  pub fn connects(&self) -> Vec<Option<u64>> {
    self.state().connects.clone()
  }

  /// The tasks read one by one so far.
  pub fn reads(&self) -> Vec<TaskId> {
    self.state().reads.clone()
  }

  /// The sequence of the last event; the cursor of a snapshot taken now.
  pub fn cursor(&self) -> u64 {
    self.state().cursor()
  }

  /// Retitles `task` without an event, as a change whose delta a cache missed.
  pub fn retitle_unannounced(&self, task: TaskId, title: &str) {
    let mut state = self.state();
    let found = state.task(task);
    found.content.title = title.to_owned();
    found.revision = next_revision(found.revision);
  }

  /// Ends `token`'s session: its streams get `revoked`, and it opens none again.
  pub fn revoke(&self, token: &str) {
    self.state().revoked.push(token.to_owned());
    self.shared.appended.notify_all();
  }

  /// Ends every open stream, as a restart of the server would.
  pub fn drop_streams(&self) {
    self.state().generation += 1;
    self.shared.appended.notify_all();
  }

  /// Answers the next `count` streams `503`.
  pub fn refuse_streams(&self, count: usize) {
    self.state().refuse_streams = count;
  }
}

impl State {
  fn cursor(&self) -> u64 {
    self.events.last().map_or(FIRST_EVENT - 1, |(id, _)| *id)
  }

  fn task(&mut self, id: TaskId) -> &mut SharedTask {
    self
      .tasks
      .iter_mut()
      .find(|task| task.id == id)
      .expect("a task of the project")
  }

  fn member(&self, token: &str) -> Option<UserRef> {
    if self.revoked.iter().any(|revoked| revoked == token) {
      return None;
    }
    self
      .members
      .iter()
      .find(|(known, _)| known == token)
      .map(|(_, user)| user.clone())
  }

  fn tick(&mut self) -> i64 {
    self.clock += 1;
    self.clock
  }

  /// Appends a delta made by `by`; returns its sequence.
  fn append<T>(&mut self, by: &UserRef, change: T, event: fn(Delta<T>) -> Event) {
    let id = self.cursor() + 1;
    let at = self.tick();
    let delta = Delta {
      id,
      by: by.clone(),
      actor: ChangeActor::Person,
      at,
      change,
    };
    self.events.push((id, sse(Some(id), &event(delta))));
  }
}

fn next_revision(revision: TaskRevision) -> TaskRevision {
  TaskRevision::try_from(revision.get() + 1).expect("a revision above zero")
}

fn task(number: u32, title: &str, by: &UserRef) -> SharedTask {
  let attribution = Attribution { by: by.clone(), at: 1 };
  SharedTask {
    id: TaskId::generate(),
    number,
    key: format!("ORC-{number}"),
    former_key: None,
    revision: TaskRevision::FIRST,
    content: TaskContent {
      title: title.to_owned(),
      brief: String::new(),
      criteria: Vec::new(),
      pipeline: None,
      limits: None,
      workflow: WorkflowStatus::Todo,
      priority: Priority::None,
      tags: Vec::new(),
      blocking: Vec::new(),
      blocked_reason: None,
      module: None,
    },
    status: TaskStatus::Draft,
    has_run: false,
    created: attribution.clone(),
    updated: attribution,
    claim: None,
    run: None,
  }
}

/// `event` as one SSE event: its kind as the name, its payload as the data.
fn sse(id: Option<u64>, event: &Event) -> String {
  let value = serde_json::to_value(event).expect("an event serializes");
  let id = id.map(|id| format!("id: {id}\n")).unwrap_or_default();
  format!(
    "{id}event: {}\ndata: {}\n\n",
    value["kind"].as_str().expect("a kind"),
    value["data"]
  )
}

/// One request: its method, path, headers with lower-cased names, and body.
struct Request {
  method: String,
  path: String,
  headers: Vec<(String, String)>,
  body: Vec<u8>,
}

impl Request {
  fn read(stream: &TcpStream) -> Option<Self> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next()?.to_owned(), parts.next()?.to_owned());
    let mut headers = Vec::new();
    loop {
      let mut header = String::new();
      reader.read_line(&mut header).ok()?;
      let header = header.trim_end();
      if header.is_empty() {
        break;
      }
      let (name, value) = header.split_once(':')?;
      headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
    }
    let length = headers
      .iter()
      .find(|(name, _)| name == "content-length")
      .and_then(|(_, value)| value.parse().ok())
      .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Self {
      method,
      path,
      headers,
      body,
    })
  }

  fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(header, _)| header == name)
      .map(|(_, value)| value.as_str())
  }

  fn token(&self) -> Option<&str> {
    self.header("authorization")?.strip_prefix("Bearer ")
  }

  fn json<T: serde::de::DeserializeOwned>(&self) -> T {
    serde_json::from_slice(&self.body).expect("a request the contract reads")
  }
}

fn respond(stream: &mut TcpStream, status: u16, body: &Value) {
  let body = body.to_string();
  let head = format!(
    "HTTP/1.1 {status} Answer\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    body.len()
  );
  // A client that hung up learns nothing; the test reads what reached it.
  let _ = stream
    .write_all(head.as_bytes())
    .and_then(|()| stream.write_all(body.as_bytes()));
}

fn refusal(stream: &mut TcpStream, status: u16, error: &str) {
  respond(stream, status, &json!({ "error": error, "message": "Refused" }));
}

fn answer(shared: &Shared, project: ProjectId, mut stream: TcpStream) {
  let Some(request) = Request::read(&stream) else {
    return;
  };
  let prefix = format!("/api/v1/projects/{project}");
  let Some(route) = request.path.strip_prefix(&prefix) else {
    return refusal(&mut stream, 404, "not_found");
  };
  let user = {
    let state = shared.state.lock().expect("state");
    request.token().and_then(|token| state.member(token))
  };
  let Some(user) = user else {
    return refusal(&mut stream, 401, "unauthorized");
  };
  let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
  match (request.method.as_str(), segments.as_slice()) {
    ("GET", ["events"]) => events(shared, &request, &mut stream),
    ("GET", ["tasks", task]) => {
      let mut state = shared.state.lock().expect("state");
      let id = task.parse().expect("a task id");
      state.reads.push(id);
      let task = state.task(id).clone();
      respond(&mut stream, 200, &json!({ "task": task }));
    }
    ("PATCH", ["tasks", task]) => change(shared, &user, task.parse().expect("a task id"), &request, &mut stream),
    ("POST", ["tasks", task, "claim"]) => claim(shared, &user, task.parse().expect("a task id"), &request, &mut stream),
    ("PUT", ["tasks", task, "run"]) => run(shared, &user, task.parse().expect("a task id"), &request, &mut stream),
    _ => refusal(&mut stream, 404, "not_found"),
  }
}

/// `PATCH …/tasks/{task}`: the title and the workflow status, made on the revision the task has.
fn change(shared: &Shared, user: &UserRef, id: TaskId, request: &Request, stream: &mut TcpStream) {
  let patch: TaskPatch = request.json();
  let mut state = shared.state.lock().expect("state");
  let at = state.tick();
  let task = state.task(id);
  if task.revision != patch.revision {
    drop(state);
    return refusal(stream, 409, "revision_conflict");
  }
  let base = task.revision;
  task.revision = next_revision(base);
  if let Some(title) = &patch.changes.title {
    task.content.title.clone_from(title);
  }
  if let Some(workflow) = patch.changes.workflow {
    task.content.workflow = workflow;
  }
  task.updated = Attribution { by: user.clone(), at };
  let confirmed = task.clone();
  let changed = TaskChanged {
    task: id,
    revision: confirmed.revision,
    base,
    title: patch.changes.title,
    brief: None,
    criteria: None,
    pipeline: None,
    limits: None,
    status: None,
    module: None,
    priority: None,
    workflow: patch.changes.workflow,
    tags_added: Vec::new(),
    tags_removed: Vec::new(),
    blocked_by: None,
    blocked_reason: None,
  };
  state.append(user, changed, Event::TaskChanged);
  drop(state);
  shared.appended.notify_all();
  respond(stream, 200, &json!({ "task": confirmed }));
}

fn claim(shared: &Shared, user: &UserRef, id: TaskId, request: &Request, stream: &mut TcpStream) {
  let take: ClaimTake = request.json();
  let mut state = shared.state.lock().expect("state");
  let at = state.tick();
  let claim = SharedClaim {
    id: ClaimId::generate(),
    holder: user.clone(),
    kind: take.kind,
    since: at,
    heartbeat_at: at,
    stale: false,
    taken_over_from: None,
  };
  let task = state.task(id);
  task.claim = Some(claim.clone());
  let confirmed = task.clone();
  let delta = TaskClaim {
    task: id,
    claim: Some(claim),
    action: ClaimAction::Taken,
  };
  state.append(user, delta, Event::TaskClaim);
  drop(state);
  shared.appended.notify_all();
  respond(stream, 200, &json!({ "task": confirmed }));
}

fn run(shared: &Shared, user: &UserRef, id: TaskId, request: &Request, stream: &mut TcpStream) {
  let report: RunReport = request.json();
  let mut state = shared.state.lock().expect("state");
  let at = state.tick();
  let task = state.task(id);
  task.has_run = true;
  task.run = Some(RunStatus {
    holder: user.clone(),
    number: report.number,
    phase: report.phase,
    step: report.step.clone(),
    started_at: report.started_at,
    phase_since: report.phase_since,
    updated_at: at,
  });
  let confirmed = task.clone();
  let delta = TaskRun {
    task: id,
    number: report.number,
    phase: report.phase,
    step: report.step.map(|step| RunStep {
      key: step.to_lowercase(),
      name: step,
    }),
    phase_since: report.phase_since,
    updated_at: at,
  };
  state.append(user, delta, Event::TaskRun);
  drop(state);
  shared.appended.notify_all();
  respond(stream, 200, &json!({ "task": confirmed }));
}

/// `GET …/events`: the snapshot without a cursor, then every delta after the cursor as it is appended, a keep-alive
/// while nothing is, until the session is revoked, the streams are dropped or the client hangs up.
fn events(shared: &Shared, request: &Request, stream: &mut TcpStream) {
  let token = request.token().unwrap_or_default().to_owned();
  let last: Option<u64> = request
    .header("last-event-id")
    .map(|id| id.parse().expect("a sequence"));
  let mut state = shared.state.lock().expect("state");
  state.connects.push(last);
  if state.refuse_streams > 0 {
    state.refuse_streams -= 1;
    drop(state);
    return refusal(stream, 503, "unavailable");
  }
  let generation = state.generation;
  let mut text = String::new();
  let mut cursor = match last {
    Some(cursor) => cursor,
    None => {
      text.push_str(&snapshot(&state));
      state.cursor()
    }
  };
  drop(state);
  let head =
    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
  if stream.write_all(head.as_bytes()).is_err() {
    return;
  }
  loop {
    if !text.is_empty() && (stream.write_all(text.as_bytes()).is_err() || stream.flush().is_err()) {
      return;
    }
    let state = shared.state.lock().expect("state");
    let (state, _) = shared
      .appended
      .wait_timeout_while(state, KEEP_ALIVE, |state| {
        state.cursor() <= cursor && state.generation == generation && state.member(&token).is_some()
      })
      .expect("state");
    if state.generation != generation {
      return;
    }
    if state.member(&token).is_none() {
      drop(state);
      // The stream closes after it whether the client read it or not.
      let _ = stream.write_all(sse(None, &Event::Revoked(orchester_api::Revoked {})).as_bytes());
      return;
    }
    text = state
      .events
      .iter()
      .filter(|(id, _)| *id > cursor)
      .map(|(_, text)| text.as_str())
      .collect();
    cursor = state.cursor();
    if text.is_empty() {
      text = ": keep-alive\n\n".to_owned();
    }
  }
}

/// The project as a member reads it now: its first page, its tasks, and the cursor the deltas follow.
fn snapshot(state: &State) -> String {
  let begin = SnapshotBegin {
    project: SharedProject {
      id: ProjectId::generate(),
      slug: "orchester".to_owned(),
      address: "https://team.example/orchester".to_owned(),
      key: "ORC".to_owned(),
      name: "Orchester".to_owned(),
      remote_hint: None,
      role: Some(MemberRole::Member),
      next_number: u64::try_from(state.tasks.len()).expect("a count") + 1,
    },
    members: Vec::new(),
    tags: state.tags.clone(),
    modules: Vec::new(),
    resources: Vec::new(),
  };
  let mut text = sse(None, &Event::SnapshotBegin(begin));
  for page in state.tasks.chunks(SnapshotTasks::PAGE) {
    text.push_str(&sse(
      None,
      &Event::SnapshotTasks(SnapshotTasks { tasks: page.to_vec() }),
    ));
  }
  text.push_str(&sse(None, &Event::SnapshotEnd(SnapshotEnd { cursor: state.cursor() })));
  text
}
