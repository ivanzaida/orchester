//! A fake team server on loopback for the live updates: one project with its tasks and its resources, the members'
//! tokens, and an event log its streams follow. It serves the snapshot to a stream without `Last-Event-ID` and the
//! deltas after the cursor to one with it, live as they are appended; lists and reads tasks; publishes and serves
//! resource versions ([`resources`]); and takes a task change, a claim and a run report, each appending its delta. Its
//! admins publish the team's configuration ([`publishing`]). It keeps each catalog report as its JSON. A member's
//! session can be revoked, streams dropped or refused for a while, and every other request refused.

mod publishing;
mod resources;

use std::{
  io::Write,
  net::{TcpListener, TcpStream},
  sync::{Arc, Condvar, Mutex, MutexGuard},
  thread,
  time::Duration,
};

use orchester_api::{
  Attribution, ClaimAction, ClaimTake, Delta, Event, RunReport, RunStatus, RunStep, SharedClaim, SharedProject,
  SharedResource, SharedTag, SharedTask, SharedVersion, SnapshotBegin, SnapshotEnd, SnapshotTasks, TaskChanged,
  TaskClaim, TaskContent, TaskPatch, TaskRun, UserRef,
};
use orchester_core::{
  blob::ContentHash,
  id::{ClaimId, ProjectId, TagId, TaskId, UserId},
  run::TaskPhase,
  sharing::{MemberRole, TaskRevision},
  task::{
    TaskStatus,
    organization::{ChangeActor, Priority, WorkflowStatus},
  },
};
use serde_json::{Value, json};

use crate::http::{Request, no_content, refusal, respond, sse};

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
  /// The tokens of who changes the team's configuration: the owner, and whom the test makes an admin.
  admins: Vec<String>,
  tasks: Vec<SharedTask>,
  tags: Vec<SharedTag>,
  resources: Vec<SharedResource>,
  versions: Vec<SharedVersion>,
  /// The skill parts uploaded, by their hash.
  files: Vec<(ContentHash, Vec<u8>)>,
  /// Each delta's sequence and its SSE text.
  events: Vec<(u64, String)>,
  /// The `Last-Event-ID` of each stream opened, in order.
  connects: Vec<Option<u64>>,
  reads: Vec<TaskId>,
  /// Each `PUT …/catalog` body, as it came.
  catalogs: Vec<Value>,
  revoked: Vec<String>,
  /// Streams opened before this generation end.
  generation: u64,
  refuse_streams: usize,
  refuse_requests: bool,
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
    let owner = members[0].0.clone();
    let shared = Arc::new(Shared {
      state: Mutex::new(State {
        members,
        admins: vec![owner],
        tasks,
        tags: vec![SharedTag {
          id: TagId::generate(),
          name: "backend".to_owned(),
        }],
        resources: Vec::new(),
        versions: Vec::new(),
        files: Vec::new(),
        events: Vec::new(),
        connects: Vec::new(),
        reads: Vec::new(),
        catalogs: Vec::new(),
        revoked: Vec::new(),
        generation: 0,
        refuse_streams: 0,
        refuse_requests: false,
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

  /// The catalog reports taken so far, each as its JSON body.
  pub fn catalogs(&self) -> Vec<Value> {
    self.state().catalogs.clone()
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

  /// Answers every request but the streams `503` while `refuse` holds.
  pub fn refuse_requests(&self, refuse: bool) {
    self.state().refuse_requests = refuse;
  }

  /// The member whose token is `token`.
  pub fn member(&self, token: &str) -> UserRef {
    self.state().member(token).expect("a member's token")
  }

  /// A task `id` titled `title` added to the project, its run claim held by `token`'s member; as the server has it.
  pub fn claimed(&self, id: TaskId, title: &str, token: &str) -> SharedTask {
    let mut state = self.state();
    let holder = state.member(token).expect("a member's token");
    let number = u32::try_from(state.tasks.len()).expect("a count") + 1;
    let mut claimed = task(number, title, &holder);
    claimed.id = id;
    claimed.claim = Some(SharedClaim {
      id: ClaimId::generate(),
      holder,
      kind: orchester_core::sharing::ClaimKind::Run,
      since: 1,
      heartbeat_at: 1,
      stale: false,
      taken_over_from: None,
    });
    state.tasks.push(claimed.clone());
    claimed
  }

  /// The workflow status `task` has on the server.
  pub fn workflow(&self, task: TaskId) -> WorkflowStatus {
    self.state().task(task).content.workflow
  }

  /// The revision `task` has on the server.
  pub fn revision(&self, task: TaskId) -> u64 {
    self.state().task(task).revision.get()
  }

  /// Whether someone holds `task`'s claim on the server.
  pub fn claimed_now(&self, task: TaskId) -> bool {
    self.state().task(task).claim.is_some()
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

fn answer(shared: &Shared, project: ProjectId, mut stream: TcpStream) {
  let Some(request) = Request::read(&stream) else {
    return;
  };
  let prefix = format!("/api/v1/projects/{project}");
  let Some(route) = request.path.strip_prefix(&prefix) else {
    return refusal(&mut stream, 404, "not_found");
  };
  let (route, query) = route.split_once('?').unwrap_or((route, ""));
  let (user, admin) = {
    let state = shared.state.lock().expect("state");
    let token = request.token().unwrap_or_default();
    (state.member(token), state.admins.iter().any(|admin| admin == token))
  };
  let Some(user) = user else {
    return refusal(&mut stream, 401, "unauthorized");
  };
  let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
  let refused = shared.state.lock().expect("state").refuse_requests;
  if refused && segments != ["events"] {
    return refusal(&mut stream, 503, "unavailable");
  }
  match (request.method.as_str(), segments.as_slice()) {
    ("GET", ["events"]) => events(shared, &request, &mut stream),
    ("GET", ["tasks"]) => {
      let state = shared.state.lock().expect("state");
      let tasks = json!({ "tasks": state.tasks, "cursor": state.cursor() });
      drop(state);
      respond(&mut stream, 200, &tasks);
    }
    ("GET", ["tasks", task]) => {
      let mut state = shared.state.lock().expect("state");
      let id = task.parse().expect("a task id");
      state.reads.push(id);
      let task = state.task(id).clone();
      respond(&mut stream, 200, &json!({ "task": task }));
    }
    ("GET", ["resources", resource, "versions"]) => {
      let page = shared.state.lock().expect("state").versions_page(resource, query);
      respond(&mut stream, 200, &page);
    }
    ("PATCH", ["tasks", task]) => change(shared, &user, task.parse().expect("a task id"), &request, &mut stream),
    ("POST", ["tasks", task, "claim"]) => claim(shared, &user, task.parse().expect("a task id"), &request, &mut stream),
    ("PUT", ["tasks", task, "run"]) => run(shared, &user, task.parse().expect("a task id"), &request, &mut stream),
    ("PUT", ["catalog"]) => {
      let report = serde_json::from_slice(request.bytes()).expect("a JSON report");
      shared.state.lock().expect("state").catalogs.push(report);
      no_content(&mut stream);
    }
    ("POST", ["resources", ..]) | ("PUT", ["files", _]) if !admin => publishing::not_admin(&mut stream),
    ("POST", ["resources"]) => publishing::create(shared, &user, &request, &mut stream),
    ("POST", ["resources", "import"]) => publishing::import(shared, &user, &request, &mut stream),
    ("POST", ["resources", resource, "versions"]) => {
      publishing::publish(shared, &user, resource, &request, &mut stream)
    }
    ("PUT", ["files", hash]) => publishing::upload(shared, hash, &request, &mut stream),
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

/// `PUT …/tasks/{task}/run`: the run's progress; a merged run's completes the task's claim.
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
  let completed = report.phase == TaskPhase::Merged && task.claim.take().is_some();
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
  if completed {
    let delta = TaskClaim {
      task: id,
      claim: None,
      action: ClaimAction::Completed,
    };
    state.append(user, delta, Event::TaskClaim);
  }
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
      text.push_str(&snapshot(&state, &token));
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

/// The project as `token`'s member reads it now: its first page with their role (the first member owns it), its tasks,
/// and the cursor the deltas follow.
fn snapshot(state: &State, token: &str) -> String {
  let owner = state.members.first().is_some_and(|(first, _)| first == token);
  let begin = SnapshotBegin {
    project: SharedProject {
      id: ProjectId::generate(),
      slug: "orchester".to_owned(),
      address: "https://team.example/orchester".to_owned(),
      key: "ORC".to_owned(),
      name: "Orchester".to_owned(),
      remote_hint: None,
      role: Some(if owner { MemberRole::Owner } else { MemberRole::Member }),
      next_number: u64::try_from(state.tasks.len()).expect("a count") + 1,
    },
    members: Vec::new(),
    tags: state.tags.clone(),
    modules: Vec::new(),
    resources: state.resources.clone(),
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
