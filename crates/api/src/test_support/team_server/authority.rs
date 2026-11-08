//! The stand-in's workspace-authority surface under `/api/v2` (the S10 reads and the S11 subscription), on the team
//! server's routes and in its shapes (`docs/workspace-reads.md`, `docs/workspace-subscriptions.md` of the platform),
//! for the app's tests and QA states that read a team project from its server: the caller's capabilities and the
//! workspace, the task list with its filters, search, sort, pages and board counts, one task, the module list, a module
//! and its progress, the releases ([`roadmap`]), and a subscription ([`stream`]) that says `ready`, sends each
//! committed change as `invalidated` and keeps the stream alive. Not served yet: a module's burndown and history, the
//! library, the picker and a resource's versions.
//! The tasks are the project's v1 tasks as [`TasksAnswer`] serves them, or the ones [`TeamServer::authority_tasks`]
//! set, with the modules [`TeamServer::authority_modules`] sets ([`work`]): a held answer holds the task reads too, and
//! a refused one answers them, as [`TeamServer::authority_unavailable`] answers every read and the subscription, `503
//! unavailable`. Who reads them stands as [`TeamServer::stand_as`] says; the v1 members read names the Admins and
//! server admins [`TeamServer::members_named`] gives.

mod commands;
mod delivery;
pub(super) mod edits;
mod execution;
mod listing;
pub(super) mod roadmap;
mod stream;
mod work;

use std::{
  collections::HashMap,
  io::Write,
  net::TcpStream,
  thread,
  time::{Duration, Instant},
};

use parking_lot::Mutex;
use serde_json::{Value, json};

pub use self::{
  commands::Plan,
  delivery::{Authorized as DeliveryAuthorized, PendingDelivery},
  execution::BundleFault,
  work::{AuthorityClaim, AuthorityModule, Person, STALE_AFTER_MINUTES},
};
use super::{State, TasksAnswer, TeamServer, USER, first_download};
use crate::{client::CLIENT_VERSION_HEADER, releases::ReleaseVersion};

/// The cursor before the first change the stand-in commits.
const FIRST_CURSOR: u64 = 7;
/// How often an open subscription looks for a change, and how long it stays silent before a keep-alive.
const STREAM_POLL: Duration = Duration::from_millis(50);
const KEEP_ALIVE: Duration = Duration::from_secs(1);

/// The caller's standing in the workspace, as its capabilities say it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Standing {
  #[default]
  Member,
  Admin,
  ServerAdmin,
}

impl Standing {
  const fn wire(self) -> &'static str {
    match self {
      Self::Member => "member",
      Self::Admin => "admin",
      Self::ServerAdmin => "server_admin",
    }
  }

  /// The person's role in a project as the v1 reads give it: none for a server admin, who is no member.
  fn v1_role(self) -> Value {
    match self {
      Self::Member => json!("member"),
      Self::Admin => json!("owner"),
      Self::ServerAdmin => Value::Null,
    }
  }
}

/// A task as the stand-in's `/api/v2` reads give it: its number in the project, title, workflow status and priority as
/// the wire names them, the resource id of its team pipeline, the module it is in, and how long ago it was created; its
/// id, when it is to be the one this computer's runs of it name, its assignee, its claim and the state of the work
/// under that claim. Its brief and its one criterion repeat its title, so it is complete.
#[derive(Clone, Debug)]
pub struct AuthorityTask {
  pub number: u32,
  pub title: String,
  pub workflow: &'static str,
  pub priority: &'static str,
  pub pipeline: Option<String>,
  /// The index of one of the modules [`TeamServer::authority_modules`] set; without them, any is the project's one.
  pub module: Option<usize>,
  pub created_minutes_ago: i64,
  /// Made from its number when `None`.
  pub id: Option<String>,
  pub assignee: Option<Person>,
  pub claim: Option<AuthorityClaim>,
  /// The work's execution state as the wire names it (`running`, `awaiting_human`); none without a claim.
  pub work: Option<&'static str>,
}

/// What the stand-in's `/api/v2` surface answers.
#[derive(Default)]
pub(super) struct Authority {
  pub(in crate::test_support::team_server) standing: Standing,
  tasks: Option<Vec<AuthorityTask>>,
  modules: Option<Vec<AuthorityModule>>,
  unavailable: bool,
  /// How every `work.start` fails to give the desktop a bundle it can use, when it does.
  bundle_fault: Option<BundleFault>,
  /// The ids of the tasks whose current work has an accepted report: a run is under it.
  reported: Vec<String>,
  /// Whether `claim.release` is refused for any running work, as a server older than the rule that lets a holder
  /// release work that started no run.
  refuses_running_releases: bool,
  /// The oldest desktop version the server accepts; any while `None`.
  required: Option<ReleaseVersion>,
  /// Each committed change: its cursor and the scopes it invalidates.
  batches: Vec<(u64, Vec<Value>)>,
  /// Streams opened before this generation end.
  streams: u64,
  /// How many task list, count and task reads were answered or are held.
  reads: usize,
  /// How many reads of one module were answered.
  module_reads: usize,
  /// The usernames the members read names as Admins and as server admins.
  admins: Vec<String>,
  server_admins: Vec<String>,
  /// Every command by its operation id: its request, its status and its answer.
  operations: Vec<(String, Value, u16, Value)>,
  /// The workspace policy: its revision and the automatic merge.
  policy: (u64, bool),
  /// An Admin's change of the automatic merge that lands before the next one asked here.
  policy_first: Option<bool>,
  /// What the next command of a kind meets in place of its rule.
  planned: Vec<(&'static str, Plan)>,
  /// The modules' branches as Admins changed them, by the module's index.
  branches: HashMap<usize, work::BranchChange>,
  /// Admins' delivery authorizations no request cited yet.
  authorizations: Vec<delivery::KeptAuthorization>,
  /// How many claims were acquired anew here.
  issued: u8,
  /// The tasks' definitions as Admins changed them, by the task's id, and someone else's change of one that lands
  /// before the next one asked here.
  edits: HashMap<String, edits::Edited>,
  edit_first: Option<edits::FirstChange>,
  /// The releases Admins made, with their changelogs and every move of a task into or out of one.
  releases: roadmap::Releases,
}

impl Authority {
  /// Nothing set yet, the policy at its first revision with the automatic merge off.
  pub(super) fn new() -> Self {
    Self {
      policy: (1, false),
      ..Self::default()
    }
  }

  fn cursor(&self) -> u64 {
    self.batches.last().map_or(FIRST_CURSOR, |(cursor, _)| *cursor)
  }

  fn commit(&mut self, scopes: Vec<Value>) {
    let cursor = self.cursor() + 1;
    self.batches.push((cursor, scopes));
  }

  /// The caller is the server's admin.
  pub(super) fn server_admin(&self) -> bool {
    self.standing == Standing::ServerAdmin
  }

  /// The caller's role in a project as the v1 reads give it.
  pub(super) fn v1_role(&self) -> Value {
    self.standing.v1_role()
  }
}

impl TeamServer {
  /// The caller stands as `standing` in every project from now on, in the v1 reads too: an Admin's role there is
  /// `owner`, and a server admin is no member of the projects, with the server's `admin` on the account a join signs
  /// in.
  pub fn stand_as(&self, standing: Standing) {
    let mut state = self.state.lock();
    state.authority.standing = standing;
    for project in &mut state.projects {
      project["role"] = standing.v1_role();
    }
  }

  /// The `/api/v2` reads give `tasks` in place of the project's v1 tasks; the change is committed as one batch.
  pub fn authority_tasks(&self, tasks: Vec<AuthorityTask>) {
    let mut state = self.state.lock();
    state.authority.tasks = Some(tasks);
    state.authority.commit(vec![json!({"kind": "task_list"})]);
  }

  /// The `/api/v2` reads give `modules` in place of the project's one module; the change is committed as one batch.
  pub fn authority_modules(&self, modules: Vec<AuthorityModule>) {
    let mut state = self.state.lock();
    state.authority.modules = Some(modules);
    state.authority.commit(vec![json!({"kind": "module_list"})]);
  }

  /// Retitles the task numbered `number` of those [`Self::authority_tasks`] set, in one committed batch that
  /// invalidates the list and the task.
  pub fn retitle(&self, number: u32, title: &str) {
    let mut state = self.state.lock();
    let tasks = state.authority.tasks.iter_mut().flatten();
    let Some(task) = tasks.into_iter().find(|task| task.number == number) else {
      return;
    };
    title.clone_into(&mut task.title);
    let id = listing::task_id(task);
    state
      .authority
      .commit(vec![json!({"kind": "task_list"}), json!({"kind": "task", "task": id})]);
  }

  /// Every `/api/v2` read and the subscription answer `503 unavailable` while `unavailable`; the open streams end.
  pub fn authority_unavailable(&self, unavailable: bool) {
    unavailable_now(&self.state, unavailable);
  }

  /// Every `work.start` names its bundle in a schema one newer than the desktop reads, as a team server a release ahead
  /// of the desktop does: the start can never become a run here, whatever the desktop does.
  pub fn authority_newer_bundles(&self) {
    self.authority_bundle_fault(BundleFault::Newer);
  }

  /// A `claim.release` of running work is refused `invalid` whatever the work, as a server older than the rule that
  /// lets the holder release work that has no accepted report.
  pub fn authority_refuses_running_releases(&self) {
    self.state.lock().authority.refuses_running_releases = true;
  }

  /// Every `work.start` fails as `fault` says, whatever the desktop does.
  pub fn authority_bundle_fault(&self, fault: BundleFault) {
    self.state.lock().authority.bundle_fault = Some(fault);
  }

  /// Every `work.start` fails as `fault` says once `after` has passed, in place of what it did before: a team server
  /// that did not answer a Run and answers it later.
  pub fn authority_bundle_fault_after(&self, fault: BundleFault, after: Duration) {
    let state = self.state.clone();
    thread::spawn(move || {
      thread::sleep(after);
      state.lock().authority.bundle_fault = Some(fault);
    });
  }

  /// [`Self::authority_unavailable`] `after` the first task read was answered: an outage in a session that read the
  /// tasks already.
  pub fn authority_unavailable_once_read(&self, after: Duration) {
    self.unavailable_once(after, |authority| authority.reads > 0);
  }

  /// [`Self::authority_unavailable`] `after` the first read of one module was answered: an outage while a module's
  /// page shows.
  pub fn authority_unavailable_once_module_read(&self, after: Duration) {
    self.unavailable_once(after, |authority| authority.module_reads > 0);
  }

  fn unavailable_once(&self, after: Duration, read: impl Fn(&Authority) -> bool + Send + 'static) {
    let state = self.state.clone();
    thread::spawn(move || {
      while !read(&state.lock().authority) {
        thread::sleep(STREAM_POLL);
      }
      thread::sleep(after);
      unavailable_now(&state, true);
    });
  }

  /// How many task list, count and task reads were answered or are held.
  pub fn authority_reads(&self) -> usize {
    self.state.lock().authority.reads
  }

  /// The members read names `admins` as the project's Admins and `server_admins` as the server's admins.
  pub fn members_named(&self, admins: &[&str], server_admins: &[&str]) {
    let mut state = self.state.lock();
    state.authority.admins = admins.iter().map(|name| (*name).to_owned()).collect();
    state.authority.server_admins = server_admins.iter().map(|name| (*name).to_owned()).collect();
  }

  /// The next command of `kind` (`claim.take_over`, `module.branch.set`, …) meets `plan` in place of its rule, once.
  pub fn plan(&self, kind: &'static str, plan: Plan) {
    self.state.lock().authority.planned.push((kind, plan));
  }

  /// The workspace's automatic merge is `on`, at the policy's next revision; committed as one batch.
  pub fn automatic_merge(&self, on: bool) {
    let mut state = self.state.lock();
    let revision = state.authority.policy.0 + 1;
    state.authority.policy = (revision, on);
    state.authority.commit(vec![json!({"kind": "workspace"})]);
  }

  /// Another Admin sets the automatic merge `on` just before the next change asked here, which then finds the policy at
  /// a newer revision.
  pub fn automatic_merge_changed_first(&self, on: bool) {
    self.state.lock().authority.policy_first = Some(on);
  }

  /// The claim on the task numbered `number` was last renewed `minutes` ago, and is `stale` or not; committed as one
  /// batch. Nothing changes for a task without a claim.
  pub fn claim_aged(&self, number: u32, minutes: i64, stale: bool) {
    let mut state = self.state.lock();
    let tasks = state.authority.tasks.iter_mut().flatten();
    let Some(claim) = tasks
      .into_iter()
      .find(|task| task.number == number)
      .and_then(|task| task.claim.as_mut())
    else {
      return;
    };
    claim.renewed_minutes_ago = minutes;
    claim.stale = stale;
    state.authority.commit(vec![json!({"kind": "task_list"})]);
  }

  /// The server accepts desktops of `required` and later only, `None` any: a `/api/v2` request that names an older
  /// version in `Orchester-Client-Version`, or none, is refused `426 unsupported_contract` with `required_version`.
  pub fn require_version(&self, required: Option<&str>) {
    let mut state = self.state.lock();
    state.authority.required = required.map(|version| ReleaseVersion::parse(version).expect("a release version"));
    state.authority.streams += 1;
  }

  /// The kind of every command asked so far (`claim.acquire`, `delivery.authorize`, …), taken or refused, in order.
  pub fn commands(&self) -> Vec<String> {
    let state = self.state.lock();
    let operations = state.authority.operations.iter();
    operations
      .map(|(_, asked, ..)| asked["command"]["kind"].as_str().unwrap_or_default().to_owned())
      .collect()
  }

  /// The workspace policy now: its revision and whether the automatic merge is on.
  pub fn policy(&self) -> (u64, bool) {
    self.state.lock().authority.policy
  }
}

/// `GET /api/v1/projects/{id}/members`: Bohdan as he stands (a member, an Admin, or one of the server admins), the
/// Admins and the server admins named.
pub(super) fn members(state: &State) -> Value {
  let person = |index: usize, name: &str| json!({"id": format!("0191c7c0-0000-7000-8000-0000000a{index:04x}"), "display_name": name});
  let bohdan = json!({"id": USER, "display_name": "Bohdan Koval"});
  let standing = state.authority.standing;
  let mut members: Vec<Value> = match standing.v1_role() {
    Value::Null => Vec::new(),
    role => vec![json!({"user": bohdan, "username": super::USERNAME, "role": role, "added_at": 1_000})],
  };
  members.extend(
    state
      .authority
      .admins
      .iter()
      .enumerate()
      .map(|(index, name)| json!({"user": person(index, name), "username": name, "role": "owner", "added_at": 1_000})),
  );
  let mut admins: Vec<Value> = state
    .authority
    .server_admins
    .iter()
    .enumerate()
    .map(|(index, name)| json!({"user": person(index + 100, name), "username": name}))
    .collect();
  if standing == Standing::ServerAdmin {
    admins.push(json!({"user": bohdan, "username": super::USERNAME}));
  }
  json!({"members": members, "admins": admins})
}

/// The `/api/v2` surface answers `503 unavailable` while `unavailable`; the open streams end.
fn unavailable_now(state: &Mutex<State>, unavailable: bool) {
  let mut state = state.lock();
  state.authority.unavailable = unavailable;
  state.authority.streams += 1;
}

/// The paths `orchester_api::client::Authority` calls for routes the team server names otherwise
/// (`docs/workspace-reads.md` of the platform), each with the server's route it is answered as. ORC-718 moves the
/// client onto the server's routes; this table then goes, and the stand-in serves the server's routes alone.
const CLIENT_PATHS: [(&str, &str); 3] = [
  ("POST tasks/query", "POST task-list"),
  ("POST tasks/counts", "POST task-counts"),
  ("POST modules/query", "POST module-list"),
];

/// Answers one `/api/v2` request on `stream`: `line` is its request line, `headers` its headers in lowercase. The
/// routes are the team server's: `GET workspace`, `GET capabilities`, `GET subscription`, `POST task-list`, `POST
/// task-counts`, `GET tasks/{task}`, `POST module-list`, `GET modules/{module}` and `GET modules/{module}/progress`,
/// the commands, an operation and the policy ([`commands`]), the heartbeat ([`execution`]), and the release reads
/// ([`roadmap`]);
/// the client's own paths for some of them are answered as those ([`CLIENT_PATHS`]). A request without the token is
/// `401` and one for a project the server has not `404`, in the common `{error, message}` envelope, as the server
/// answers them; an outage is `503 unavailable` in the authority envelope.
pub(super) fn serve(stream: &mut TcpStream, (line, headers, body): (&str, &str, &str), state: &Mutex<State>) {
  let target = line.split_whitespace().take(2).collect::<Vec<_>>();
  let (Some(method), Some(path)) = (target.first(), target.get(1)) else {
    return;
  };
  let required = state.lock().authority.required.clone();
  if let Some(required) = required.filter(|required| !named_at_least(headers, required)) {
    let details = json!({"required_version": required.as_str()});
    return respond(
      stream,
      426,
      &json!({"code": "unsupported_contract", "details": details}),
    );
  }
  if !headers.contains(&format!("authorization: bearer {}", super::TOKEN)) {
    return respond(stream, 401, &super::error("unauthorized"));
  }
  let Some((project, path)) = path
    .strip_prefix("/api/v2/projects/")
    .and_then(|rest| rest.split_once('/'))
  else {
    return respond(stream, 404, &super::error("not_found"));
  };
  let Some(key) = project_key(state, project) else {
    return respond(stream, 404, &super::error("not_found"));
  };
  if state.lock().authority.unavailable {
    return refusal(stream, 503, "unavailable");
  }
  let asked = format!("{method} {path}");
  let route = CLIENT_PATHS
    .iter()
    .find(|(client, _)| *client == asked)
    .map_or(asked.as_str(), |(_, server)| server);
  let project = (project, key.as_str());
  match route.split_once(' ').unwrap_or_default() {
    ("GET", "subscription") => stream::subscribe(stream, headers, state),
    ("GET", "capabilities") => respond(stream, 200, &capabilities(project.0, state.lock().authority.standing)),
    ("GET", "workspace") => respond(stream, 200, &workspace(project, state)),
    ("POST", read @ ("task-list" | "task-counts")) => task_read(stream, (read, body), project, state),
    ("GET", task) if task.starts_with("tasks/") => task_read(stream, (task, body), project, state),
    ("POST", "module-list") => respond(stream, 200, &listing::modules(project, &state.lock())),
    ("GET", module) if module.starts_with("modules/") => {
      let mut state = state.lock();
      state.authority.module_reads += 1;
      match listing::module(project, module, &state) {
        Some(answer) => respond(stream, 200, &answer),
        None => respond(stream, 404, &super::error("not_found")),
      }
    }
    ("POST", "commands") => {
      let (status, answer) = commands::command(&mut state.lock().authority, project.0, body);
      respond(stream, status, &answer);
    }
    ("GET", operation) if operation.starts_with("operations/") => {
      let id = operation.trim_start_matches("operations/");
      let (status, answer) = commands::operation(&state.lock().authority, id);
      respond(stream, status, &answer);
    }
    ("POST", "heartbeat") => {
      let (status, answer) = execution::heartbeat(&mut state.lock().authority, body);
      respond(stream, status, &answer);
    }
    ("GET", bundle)
      if bundle.starts_with("bundles/") && state.lock().authority.bundle_fault == Some(BundleFault::Unreadable) =>
    {
      respond(stream, 200, &json!({"files": "not an execution bundle"}))
    }
    ("GET", "policy") => respond(stream, 200, &commands::policy(&state.lock().authority)),
    ("PATCH", "policy") => {
      let (status, answer) = commands::change_policy(&mut state.lock().authority, body);
      respond(stream, status, &answer);
    }
    (method, read) if read.starts_with("release") => {
      let (status, answer) = roadmap::read(project, (method, read, body), &state.lock());
      respond(stream, status, &answer);
    }
    _ => respond(stream, 404, &super::error("not_found")),
  }
}

/// The key of the shared project with `id`; `None` for no project of the server.
fn project_key(state: &Mutex<State>, id: &str) -> Option<String> {
  let state = state.lock();
  let project = state.projects.iter().find(|project| project["id"] == id)?;
  project["key"].as_str().map(str::to_owned)
}

/// Whether `headers` (lowercase) name a desktop version of `required` or later in `Orchester-Client-Version`.
fn named_at_least(headers: &str, required: &ReleaseVersion) -> bool {
  let name = format!("{}:", CLIENT_VERSION_HEADER.to_ascii_lowercase());
  headers
    .lines()
    .find_map(|line| line.strip_prefix(name.as_str()))
    .and_then(|named| ReleaseVersion::parse(named.trim()).ok())
    .is_some_and(|named| named >= *required)
}

/// A task list, count or task read, held while the project's tasks are and refused while they are.
fn task_read(stream: &mut TcpStream, (route, body): (&str, &str), project: (&str, &str), state: &Mutex<State>) {
  state.lock().authority.reads += 1;
  let deadline = Instant::now() + first_download::HOLD_AT_MOST;
  let held = || {
    let state = state.lock();
    state.tasks == TasksAnswer::Hold && state.authority.tasks.is_none()
  };
  while held() && Instant::now() < deadline {
    thread::sleep(first_download::HOLD_POLL);
  }
  let state = state.lock();
  if state.authority.tasks.is_none() && matches!(state.tasks, TasksAnswer::Hold | TasksAnswer::Fail) {
    drop(state);
    return refusal(stream, 503, "unavailable");
  }
  let tasks = listing::tasks(project, &state);
  let cursor = state.authority.cursor();
  drop(state);
  let answer = match route {
    "task-list" => listing::page(&tasks, body, cursor),
    "task-counts" => listing::counts(&tasks, body, cursor),
    _ => {
      let id = route.trim_start_matches("tasks/");
      match tasks.iter().find(|task| task["id"] == id) {
        Some(task) => Ok(task.clone()),
        None => return respond(stream, 404, &super::error("not_found")),
      }
    }
  };
  match answer {
    Ok(answer) => respond(stream, 200, &answer),
    Err((status, code)) => refusal(stream, status, code),
  }
}

/// `{project_id, revision, role, actions, unresolved}` for a caller standing as `standing`, as the server lists them:
/// `read`, `export.shared` held back by decision D9, and `server.admin`, a server admin's.
pub(super) fn capabilities(project: &str, standing: Standing) -> Value {
  let action = |action: &str, reason: Option<&str>| json!({"action": action, "subject": null, "allowed": reason.is_none(), "reason": reason});
  let server_admin = (standing != Standing::ServerAdmin).then_some("role_required");
  json!({"project_id": project, "revision": 1, "role": standing.wire(),
    "actions": [action("read", None), action("export.shared", Some("decision_required")),
      action("server.admin", server_admin)],
    "unresolved": ["D9"]})
}

fn workspace((id, key): (&str, &str), state: &Mutex<State>) -> Value {
  let state = state.lock();
  let project = state
    .projects
    .iter()
    .find(|project| project["id"] == id)
    .cloned()
    .unwrap_or(Value::Null);
  let standing = state.authority.standing;
  let role = match standing {
    Standing::Member => json!("member"),
    Standing::Admin => json!("admin"),
    Standing::ServerAdmin => Value::Null,
  };
  json!({
    "project": {"id": id, "slug": project["slug"], "address": project["address"], "key": key, "name": project["name"],
      "remote_hint": project["remote_hint"], "role": role, "next_number": project["next_number"]},
    "policy": commands::policy_wire(&state.authority),
    "capabilities": capabilities(id, standing),
    "cursor": state.authority.cursor().to_string(),
    "claim_staleness_seconds": STALE_AFTER_MINUTES * 60,
  })
}

fn respond(stream: &mut TcpStream, status: u16, body: &Value) {
  let body = body.to_string();
  let reply = format!(
    "HTTP/1.1 {status} Answer\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
    body.len()
  );
  // The test reads the answer or fails on its own; a broken connection needs nothing here.
  let _ = stream.write_all(reply.as_bytes());
}

/// The authority refusal `{code, details}` with `status`.
fn refusal(stream: &mut TcpStream, status: u16, code: &str) {
  respond(stream, status, &json!({"code": code, "details": {}}));
}
