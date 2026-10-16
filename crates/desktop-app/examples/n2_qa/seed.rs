//! What each N2 state starts from: a team server with Alice Moreau (its admin, Kontur's owner), Oleh Petrenko and
//! Maria Lind, and Kontur (KON, set up for `github.com/acme/kontur`); this computer joined it as Ivan Zaida through a
//! port the seed can take down; and Kontur here in a git repository with that `origin`, linked or not as the state
//! needs. Fixture data for QA, never shipped.

use std::{error::Error, path::Path, process::Command};

use orchester_api::{
  SharedProject,
  client::{PlainHttp, ServerAddress},
};
use orchester_app::{
  routes::{ProjectPage, Route, TaskList},
  services::Services,
};
use orchester_core::{
  id::{IdempotencyKey, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  store::TaskStore,
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  project_setup::NewProject,
  sharing::{CodeRequest, Trust, project::first_link::LinkRequest},
  workspace::Workspace,
};
use serde_json::json;
use tokio::runtime::Runtime;

use super::{State, forwarder::Forwarder, server_support::TestServer};

/// Kontur's remote, the one the team server's Kontur names as its hint.
const KONTUR_REMOTE: &str = "https://github.com/acme/kontur.git";

/// The local tasks a first link uploads, and the tasks Alice's computer made on the server.
const LOCAL: [&str; 5] = [
  "Add retry to webhook sender",
  "Show delivery status on the endpoint page",
  "Paginate the webhook log",
  "Rate-limit messages from GitHub",
  "Export invoices to PDF",
];
const SERVER: [&str; 3] = [
  "Typed errors per module",
  "Show the signed-in account",
  "Retry failed exports every night",
];

/// What the window needs while it runs: the servers, which stop when this is dropped, where it opens, and what the
/// example's own presses need.
pub struct Seeded {
  pub route: Route,
  pub project: ProjectId,
  /// The project's address, through the port.
  pub address: String,
  pub server: TestServer,
  pub kontur: SharedProject,
  pub port: Forwarder,
  _elsewhere: Option<TestServer>,
}

/// Seeds `state` in `folder`.
pub fn seed(
  state: State,
  (services, workspace): (&Services, &Workspace),
  (runtime, folder): (&Runtime, &Path),
) -> Result<Seeded, Box<dyn Error>> {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  server.create_user("oleh", "Oleh Petrenko", false);
  server.create_user("maria", "Maria Lind", false);
  let kontur = server.create_named_project(("Kontur", "KON"), "kontur", Some(KONTUR_REMOTE));
  server.add_member(&kontur, "oleh");
  server.add_member(&kontur, "maria");
  let port = Forwarder::start(server.address);
  let repo = folder.join("repo-kontur");
  std::fs::create_dir_all(&repo)?;
  git(&repo, &["init", "-q"])?;
  git(&repo, &["remote", "add", "origin", KONTUR_REMOTE])?;
  let project = Project::new(ProjectId::generate(), ProjectKey::new("KON")?, Name::new("Kontur")?);
  workspace.create(NewProject {
    project: project.clone(),
    repo_root: repo.clone(),
  })?;
  if !matches!(state, State::Download | State::MissingPipeline) {
    local_tasks(&repo)?;
  }
  if matches!(state, State::Download | State::Review | State::MissingPipeline) {
    let pipeline = (state == State::MissingPipeline).then_some("kontur-delivery");
    let contents: Vec<_> = SERVER
      .iter()
      .map(|title| json!({ "title": title, "pipeline": pipeline }))
      .collect();
    server.add_tasks(&kontur, &contents);
  }
  join(&server, &kontur, (services, runtime), &port)?;
  let elsewhere = (state == State::DiscoveryUnconnected).then(TestServer::start);
  let address = format!("{}/kontur", port.url());
  let seeded = Seeded {
    route: Route::Project(project.id(), ProjectPage::Sharing { make_local: false }),
    project: project.id(),
    address,
    server,
    kontur,
    port,
    _elsewhere: elsewhere,
  };
  linked(state, seeded, (services, runtime), &repo)
}

/// Links and changes what `state` needs once this computer joined; where the window opens.
fn linked(
  state: State,
  mut seeded: Seeded,
  (services, runtime): (&Services, &Runtime),
  repo: &Path,
) -> Result<Seeded, Box<dyn Error>> {
  let (server, kontur, project) = (&seeded.server, &seeded.kontur, seeded.project);
  let tasks = Route::Project(project, ProjectPage::Tasks(TaskList::default()));
  match state {
    State::Upload | State::Download | State::Review => return Ok(seeded),
    State::Discovery => {
      std::fs::write(
        repo.join("orchester.toml"),
        format!("project = \"{}\"\n", seeded.address),
      )?;
      return Ok(seeded);
    }
    State::DiscoveryUnconnected => {
      let elsewhere = seeded._elsewhere.as_ref().ok_or("the other server")?;
      std::fs::write(
        repo.join("orchester.toml"),
        format!("project = \"{}/atlas\"\n", elsewhere.url()),
      )?;
      return Ok(seeded);
    }
    _ => {}
  }
  if matches!(
    state,
    State::Owner | State::Offline | State::MakeLocal | State::MakeLocalOffline
  ) {
    server.set_role(kontur, "ivan", "owner");
  }
  link(services, runtime, repo)?;
  if matches!(state, State::MakeLocal | State::MakeLocalOffline) {
    hold_claims(services, runtime, repo)?;
  }
  // The members as read once, which Sharing shows as last read while the server is away, and once removed.
  let open = orchester_workspace::sharing::project::ProjectServer::open(&services.servers, repo)?.ok_or("linked")?;
  runtime.block_on(open.current_members())?;
  seeded.route = match state {
    State::Offline | State::MakeLocalOffline | State::TasksOffline => {
      seeded.port.down();
      offline_route(state, project, &tasks)
    }
    State::MakeLocal => Route::Project(project, ProjectPage::Sharing { make_local: true }),
    State::Removed | State::TasksRemoved => {
      server.remove_member(kontur, "ivan");
      runtime.block_on(open.sync())?;
      if state == State::Removed {
        seeded.route.clone()
      } else {
        tasks
      }
    }
    State::TasksSignedOut => {
      let entry = services.servers.entries()?.remove(0);
      runtime.block_on(services.servers.sign_out(&entry))?;
      tasks
    }
    State::Tasks => tasks,
    State::Conflict => Route::Project(project, ProjectPage::EditTask("KON-1".parse()?)),
    State::MissingPipeline => Route::Project(project, ProjectPage::Task("KON-1".parse()?, Default::default())),
    _ => seeded.route.clone(),
  };
  Ok(seeded)
}

fn offline_route(state: State, project: ProjectId, tasks: &Route) -> Route {
  match state {
    State::MakeLocalOffline => Route::Project(project, ProjectPage::Sharing { make_local: true }),
    State::TasksOffline => tasks.clone(),
    _ => Route::Project(project, ProjectPage::Sharing { make_local: false }),
  }
}

/// Ivan joins Kontur with a project code, through `port`.
fn join(
  server: &TestServer,
  kontur: &SharedProject,
  (services, runtime): (&Services, &Runtime),
  port: &Forwarder,
) -> Result<(), Box<dyn Error>> {
  let request = CodeRequest {
    code: server.project_code(kontur),
    username: Some("ivan".into()),
    display_name: Some("Ivan Zaida".into()),
  };
  let address = ServerAddress::parse(&port.url())?;
  runtime.block_on(
    services
      .servers
      .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request),
  )?;
  Ok(())
}

/// Links Kontur here to Kontur on the server, as the person's *Link and upload* (or *Link*) on its page would.
fn link(services: &Services, runtime: &Runtime, repo: &Path) -> Result<(), Box<dyn Error>> {
  let entry = services.servers.entries()?.remove(0);
  runtime.block_on(async {
    let kontur = services.servers.find_project(&entry, "kontur").await?;
    let plan = services.servers.link_plan(&entry, repo, kontur).await?;
    let request = LinkRequest {
      remote: None,
      key: IdempotencyKey::generate(),
      write_discovery: true,
    };
    services.servers.link_checkout(&entry, repo, &plan, request).await?;
    Ok::<_, Box<dyn Error>>(())
  })
}

/// Ivan takes the run claims of KON-1 and KON-2, as starting work on them would, so Make local names them.
fn hold_claims(services: &Services, runtime: &Runtime, repo: &Path) -> Result<(), Box<dyn Error>> {
  let tasks = ProjectDb::open(repo)?.tasks()?;
  for task in tasks.iter().take(2) {
    let claim = runtime
      .block_on(services.servers.claim_for_work(repo, task.id()))
      .map_err(|refusal| format!("{refusal:?}"))?;
    // The claim stays Ivan's, as work under way would keep it.
    drop(claim);
  }
  Ok(())
}

fn local_tasks(repo: &Path) -> Result<(), Box<dyn Error>> {
  let mut db = ProjectDb::open(repo)?;
  let details = LOCAL
    .iter()
    .map(|title| {
      Ok(TaskDetails {
        title: Name::new(*title)?,
        brief: String::new(),
        acceptance_criteria: Vec::new(),
        pipeline: None,
        limits: TaskLimits::NONE,
      })
    })
    .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
  db.create_tasks(IdempotencyKey::generate(), details, Timestamp::now())?;
  Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Result<(), Box<dyn Error>> {
  let status = Command::new("git").args(args).current_dir(repo).status()?;
  if !status.success() {
    return Err(format!("git {args:?} failed").into());
  }
  Ok(())
}
