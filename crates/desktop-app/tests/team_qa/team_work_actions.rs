//! The rest of a team project's actions (ORC-732, `Team authority · redesign` `BZc66` rows 2–5 and 7), each through the
//! dispatcher to the stand-in team server: a task's Release, and its Run, which takes the claim and makes the start's
//! choices through `team_work` before `work.start`; a module's Release and Claim; Ask for delivery, which waits for an
//! Admin while Automatic merge is off; an Admin's Authorize, which asks for exactly that delivery on it; and Ask again
//! after a refused one. What only a person does takes no MCP client's press: Take over of a task or a module, Authorize
//! and the Automatic merge switch say so under the button and send nothing. Kontur's checkout is made a repository of
//! its own first, so git never reaches the repository of a folder above it.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_work`

use std::{fs, path::Path, process::Command};

use lurq::app::events::with_synthetic_input;
use orchester_api::test_support::team_server::{Plan, TeamServer};
use orchester_app::{
  routes::{ModuleTab, ProjectPage, ProjectSettingsTab, Route, TaskView},
  team::{
    act::Acting,
    start::{StartTask, Started, start},
  },
  test_support::{Window, project_id},
};
use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, ModuleId, ResourceId},
  name::Name,
  project::TaskKey,
  resource::ResourceKind,
  sharing::TaskRevision,
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{not_ready::on_task_after_setup, prepared, team::State};

/// The modules the QA app's server sets, by their index: Webhooks v2, Billing exports and API rate limits.
const WEBHOOKS: &str = "0191c7c0-0000-7000-8000-000000200000";
const BILLING: &str = "0191c7c0-0000-7000-8000-000000200001";
const RATE_LIMITS: &str = "0191c7c0-0000-7000-8000-000000200002";
/// KON-5, in API rate limits, which nobody holds in `team-claims`.
const KON_5: &str = "0191c7c0-0000-7000-8000-000000100005";
/// The committer of the checkout's commits.
const COMMITTER: [&str; 4] = ["-c", "user.name=QA", "-c", "user.email=qa@example.invalid"];

pub(super) fn task_route(key: &str) -> Route {
  let key: TaskKey = key.parse().expect("a task key");
  Route::Project(project_id(), ProjectPage::task(key, TaskView::Summary, false))
}

fn module_route(id: &str) -> Route {
  let module: ModuleId = id.parse().expect("a module id");
  Route::Project(project_id(), ProjectPage::Module(module, ModuleTab::Summary))
}

fn sharing() -> Route {
  Route::Project(project_id(), ProjectPage::Settings(ProjectSettingsTab::Sharing))
}

fn git(root: &Path, args: &[&str]) {
  let status = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .status()
    .expect("git runs");
  assert!(status.success(), "git {args:?}");
}

/// Makes Kontur's checkout at `root` a repository of its own, on `main` with a commit, `module/billing-exports` one
/// commit ahead of it and `module/api-rate-limits` at it; the project's own folder is no change of it.
pub(super) fn own_repository(root: &Path) {
  git(root, &["init", "--quiet", "--initial-branch=main"]);
  let top = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(["rev-parse", "--show-toplevel"])
    .output()
    .expect("git runs");
  let top = String::from_utf8(top.stdout).expect("a path");
  assert_eq!(
    Path::new(top.trim()).canonicalize().expect("the top"),
    root.canonicalize().expect("the checkout"),
    "the checkout is its own repository"
  );
  fs::write(root.join(".git/info/exclude"), ".orchester/\n").expect("the exclude file");
  let commit = |message: &str| {
    git(
      root,
      &[&COMMITTER[..], &["commit", "--allow-empty", "--quiet", "-m", message]].concat(),
    )
  };
  commit("base");
  git(root, &["checkout", "--quiet", "-b", "module/billing-exports"]);
  commit("export invoices");
  git(root, &["checkout", "--quiet", "main"]);
  git(root, &["branch", "module/api-rate-limits"]);
}

/// The window in `state` on Kontur's own repository, on `route` once `shown` shows.
pub(super) fn on(state: State, route: Route, shown: &str) -> (Window, TeamServer) {
  let (mut window, server) = prepared(state);
  own_repository(&window.data.repo("repo-kontur"));
  window.navigation().go(route);
  window.until(shown, |window| window.texts(shown).is_some());
  (window, server)
}

/// `id`'s words, once they start with `starts`.
fn says(window: &mut Window, id: &str, starts: &str) -> String {
  window.until(starts, |window| {
    window.texts(id).is_some_and(|texts| texts.concat().starts_with(starts))
  });
  window.text(id)
}

fn asked(server: &TeamServer, kind: &str) -> usize {
  server.commands().iter().filter(|asked| *asked == kind).count()
}

#[test]
fn team_qa_work_release_lets_a_task_go_and_claim_takes_it_again() {
  let (mut window, server) = on(State::Claims, task_route("KON-21"), "task-more");
  window.press("task-more");
  window.until("More actions", |window| window.texts("task-release-claim").is_some());
  window.press("task-release-claim");
  says(&mut window, "task-team-claim-title", "Nobody holds KON-21");
  assert_eq!(asked(&server, "claim.release"), 1);
  window.press("task-team-claim-take");
  says(&mut window, "task-team-claim-title", "Claimed by you");
}

/// The team's Team quick fix runs Reviewer, which needs only a Claude Code connection on this computer: with one added
/// the pipeline is ready here, so a start gets past the readiness it checks first (ORC-840) and takes the claim.
fn ready_pipeline(root: &Path) -> ResourceId {
  let mut db = ProjectDb::open(root).expect("the project database");
  let connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · QA").expect("a name"),
    AgentProvider::ClaudeCodeCli,
    std::env::current_exe().expect("a program that exists"),
    Timestamp::now(),
  )
  .expect("a connection");
  db.create_connection(&connection).expect("a connection");
  db.resources(ResourceKind::Pipeline)
    .expect("pipelines")
    .into_iter()
    .find(|pipeline| pipeline.name().as_str() == "Team quick fix")
    .expect("the team's Team quick fix")
    .id()
}

/// Run takes the claim through the dispatcher and makes the start's choices through `team_work`: the pipeline is
/// checked ready here first, then the claim is taken, then the module branch's tip here is the start's base and
/// `work.start` is asked (`team_work`'s own tests cover the start from there on).
#[test]
fn team_qa_work_run_takes_the_claim_and_makes_the_starts_choices() {
  let pipeline = std::cell::Cell::new(None);
  let (mut window, server, _) = on_task_after_setup(("team-not-ready", State::Claims), "KON-5", |root| {
    pipeline.set(Some(ready_pipeline(root)));
  });
  window.until("the claim line", |window| {
    window.texts("task-team-claim-title").is_some()
  });
  says(&mut window, "task-team-claim-title", "Nobody holds KON-5");
  let services = window.launch.services().expect("the session started").clone();
  let root = window.data.repo("repo-kontur");
  let runtime = window.data.runtime.handle().clone();
  let source = runtime
    .block_on(services.team.source(&root))
    .expect("Kontur's reads")
    .expect("Kontur is a team project");
  let request = StartTask {
    task: (KON_5.parse().expect("a task id"), TaskRevision::FIRST),
    claim: None,
    module: Some((
      RATE_LIMITS.parse().expect("a module id"),
      TaskRevision::FIRST,
      "module/api-rate-limits".to_owned(),
    )),
    pipeline: pipeline.get(),
    project: "Kontur".to_owned(),
  };
  let claims = asked(&server, "claim.acquire");
  let means = (services.runners.clone(), services.programs.clone());
  let started = runtime.block_on(start(&Acting::of(&services), &source, means, request));
  // The stand-in takes the work but its run is not carried out on this computer, so the start waits for recovery.
  assert_eq!(started, Started::Waiting);
  assert_eq!(asked(&server, "claim.acquire"), claims + 1);
  assert_eq!(
    asked(&server, "work.start"),
    1,
    "the start asked the server for its work once"
  );
  says(&mut window, "task-team-claim-title", "Claimed by you");
}

#[test]
fn team_qa_work_a_modules_release_and_claim_go_to_the_server() {
  let (mut window, server) = on(State::ModuleWork, module_route(BILLING), "module-team-release");
  window.press("module-team-release");
  says(&mut window, "module-team-claim-title", "Nobody holds Billing exports");
  window.press("module-team-take");
  says(
    &mut window,
    "module-team-claim-title",
    "Claimed by you on this computer",
  );
  assert_eq!(asked(&server, "claim.release"), 1);
  assert_eq!(
    asked(&server, "claim.acquire"),
    2,
    "the state's own claim, then this one"
  );
}

#[test]
fn team_qa_work_ask_for_delivery_waits_for_an_admin_while_automatic_merge_is_off() {
  let (mut window, server) = on(State::ModuleWork, module_route(BILLING), "module-team-delivery-ask");
  window.press("module-team-delivery-ask");
  let title = says(&mut window, "module-team-delivery-title", "Delivery requested · waits");
  assert_eq!(title, "Delivery requested · waits for an Admin");
  assert_eq!(asked(&server, "delivery.request"), 1);
  assert!(
    window.texts("module-team-authorize").is_none(),
    "a member authorizes nothing"
  );
}

#[test]
fn team_qa_work_ask_again_asks_once_more_after_a_refused_delivery() {
  let (mut window, server) = on(State::ModuleWork, module_route(BILLING), "module-team-delivery-ask");
  server.plan("delivery.request", Plan::Refuse(409, "claim_lost"));
  window.press("module-team-delivery-ask");
  says(&mut window, "module-team-delivery-title", "Delivery refused");
  window.press("module-team-delivery-again");
  says(
    &mut window,
    "module-team-delivery-title",
    "Delivery requested · waits for an Admin",
  );
  assert_eq!(asked(&server, "delivery.request"), 2);
}

#[test]
fn team_qa_work_an_admin_authorizes_the_delivery_that_waits_and_it_is_delivered() {
  let (mut window, server) = on(State::AdminWork, module_route(BILLING), "module-team-delivery-ask");
  server.automatic_merge(false);
  window.press("module-team-delivery-ask");
  says(
    &mut window,
    "module-team-delivery-title",
    "Delivery to main waits for your authorization",
  );

  // The server's read can show the waiting delivery while this ask still settles here, its Authorize under way: the
  // client presses until the button takes the press.
  window.until("the press to be refused", |window| {
    with_synthetic_input(|| window.press("module-team-authorize"));
    window.texts("module-team-authorize-person-only").is_some()
  });
  assert_eq!(
    asked(&server, "delivery.authorize"),
    0,
    "an MCP client authorizes nothing"
  );

  window.press("module-team-authorize");
  says(&mut window, "module-team-delivery-title", "Delivered to main");
  let commands = server.commands();
  let authorized = commands.iter().position(|kind| kind == "delivery.authorize");
  let requested = commands.iter().rposition(|kind| kind == "delivery.request");
  assert!(authorized < requested, "{commands:?}");
  assert_eq!(asked(&server, "delivery.report"), 1);
}

/// olha's request waits for an Admin (04e `GujCo`): the Admin reads who asked and when, and Authorize authorizes
/// exactly those commits; olha's own computer asks for the merge then, so nothing is requested here.
#[test]
fn team_qa_work_an_admin_reads_who_asked_and_authorizes_that_delivery() {
  let (mut window, server) = on(State::AdminWork, module_route(WEBHOOKS), "module-team-authorize");
  let title = says(&mut window, "module-team-delivery-title", "Delivery to main waits");
  assert_eq!(title, "Delivery to main waits for your authorization");
  // The branch it lands on is read from this checkout after the server's read shows the line.
  window.until("the base branch", |window| {
    window.text("module-team-delivery-detail").contains(" into main at ")
  });
  let detail = window.text("module-team-delivery-detail");
  assert!(detail.starts_with("olha asked at "), "{detail}");
  assert!(
    detail.contains("to merge module/webhooks-v2 at 4be19c2 into main at 91d0a7e."),
    "{detail}"
  );
  window.press("module-team-authorize");
  says(
    &mut window,
    "module-team-delivery-title",
    "An Admin authorized the delivery",
  );
  assert_eq!(asked(&server, "delivery.authorize"), 1);
  assert_eq!(asked(&server, "delivery.request"), 0, "olha asks for the merge herself");
  assert!(
    window.texts("module-team-delivery-again").is_none(),
    "not this computer's to merge"
  );
}

#[test]
fn team_qa_work_a_tasks_take_over_takes_no_mcp_press() {
  let (mut window, server) = on(State::Claims, task_route("KON-16"), "task-team-take-over");
  window.press("task-team-take-over");
  window.until("the question", |window| window.texts("task-take-over-title").is_some());
  with_synthetic_input(|| window.press("task-take-over-confirm"));
  window.until("the press to be refused", |window| {
    window.texts("task-take-over-confirm-person-only").is_some()
  });
  assert!(
    window.texts("task-take-over-title").is_some(),
    "the question stays open"
  );
  assert_eq!(asked(&server, "claim.take_over"), 0);
}

#[test]
fn team_qa_work_a_modules_take_over_takes_no_mcp_press() {
  let (mut window, server) = on(State::ModuleWork, module_route(WEBHOOKS), "module-team-take-over");
  with_synthetic_input(|| window.press("module-team-take-over"));
  window.until("the press to be refused", |window| {
    window.texts("module-team-take-over-person-only").is_some()
  });
  assert_eq!(asked(&server, "claim.take_over"), 0);
  assert!(window.texts("module-team-refused").is_none());
}

#[test]
fn team_qa_work_the_automatic_merge_switch_takes_no_mcp_press() {
  let (mut window, server) = on(State::AdminWork, sharing(), "sharing-policy-merge-switch");
  let before = server.policy();
  with_synthetic_input(|| window.press("sharing-policy-merge-switch"));
  window.until("the press to be refused", |window| {
    window.texts("sharing-policy-merge-switch-person-only").is_some()
  });
  assert_eq!(server.policy(), before, "the policy is as it was");
  assert!(window.texts("sharing-policy-conflict").is_none());
}
