//! What each N2 state starts from: a team server with Alice Moreau (its admin, Kontur's owner), Oleh Petrenko and
//! Maria Lind, and Kontur (KON, set up for `github.com/acme/kontur`); this computer joined it as Ivan Zaida through a
//! port the seed can take down; and Kontur here in a git repository with that `origin`, linked or not as the state
//! needs. Fixture data for QA, never shipped. `link-foreign` uses a server that names another address for its
//! projects; `no-server` links Kontur through another computer's list of servers, which this one doesn't share.

use std::{
  error::Error,
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
};

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
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  project_setup::NewProject,
  secrets::Vault,
  sharing::{
    CodeRequest, Servers, Trust,
    project::{ProjectServer, first_link::LinkRequest},
  },
  workspace::Workspace,
};
use serde_json::json;
use tokio::runtime::Runtime;

use super::{State, forwarder::Forwarder, server_support::TestServer, vault::QaVault};

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
  /// Kontur's checkout here.
  pub repo: PathBuf,
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
  let server = if state == State::LinkForeign {
    TestServer::start_announcing_another_address()
  } else {
    TestServer::start()
  };
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
  if state == State::NoServer {
    linked_elsewhere(&server, &kontur, (runtime, folder), (&port, &repo))?;
  } else {
    join(&server, &kontur, (&services.servers, runtime), &port)?;
  }
  let elsewhere = (state == State::DiscoveryUnconnected).then(TestServer::start);
  let address = format!("{}/kontur", port.url());
  let seeded = Seeded {
    route: Route::Project(project.id(), ProjectPage::Sharing { make_local: false }),
    project: project.id(),
    address,
    repo: repo.clone(),
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
    State::Upload | State::Download | State::Review | State::UploadChanged | State::UploadPersonOnly => {
      return Ok(seeded);
    }
    State::LinkForeign | State::NoServer => return Ok(seeded),
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
    State::Owner
      | State::Offline
      | State::MakeLocal
      | State::MakeLocalOffline
      | State::MembersSignedOut
      | State::DiscoveryForeign
  ) {
    server.set_role(kontur, "ivan", "owner");
  }
  link(&services.servers, runtime, repo, state != State::DiscoveryForeign)?;
  if matches!(
    state,
    State::MakeLocal | State::MakeLocalOffline | State::MakeLocalSignedOut | State::MadeLocalStale
  ) {
    hold_claims(services, runtime, repo)?;
  }
  // The members as read once, which Sharing shows as last read while the server is away, and once removed.
  let open = ProjectServer::open(&services.servers, repo)?.ok_or("linked")?;
  runtime.block_on(open.current_members())?;
  // The app's first sync, which keeps the repository hint as last read.
  runtime.block_on(open.sync())?;
  if let Some(route) = changed_after(state, &seeded, (services, runtime), &open)? {
    seeded.route = route;
    return Ok(seeded);
  }
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

/// Where the window opens for the states of the review's fixes, after what they change once linked; `None` for the
/// others.
fn changed_after(
  state: State,
  seeded: &Seeded,
  (services, runtime): (&Services, &Runtime),
  open: &ProjectServer,
) -> Result<Option<Route>, Box<dyn Error>> {
  let sharing = Route::Project(seeded.project, ProjectPage::Sharing { make_local: false });
  let make_local = Route::Project(seeded.project, ProjectPage::Sharing { make_local: true });
  let sign_out = || -> Result<(), Box<dyn Error>> {
    let entry = services.servers.entries()?.remove(0);
    runtime.block_on(services.servers.sign_out(&entry))?;
    Ok(())
  };
  Ok(Some(match state {
    State::MembersSignedOut => {
      sign_out()?;
      sharing
    }
    State::RemovedSignedOut => {
      seeded.server.remove_member(&seeded.kontur, "ivan");
      runtime.block_on(open.sync())?;
      sign_out()?;
      sharing
    }
    State::MakeLocalSignedOut => {
      sign_out()?;
      make_local
    }
    State::MadeLocalStale => make_local,
    State::DiscoveryForeign => {
      // A link made before addresses were checked against their server.
      ProjectDb::open(&seeded.repo)?.record_project_address("https://orchester.elsewhere.example/kontur")?;
      sharing
    }
    _ => return Ok(None),
  }))
}

/// Kontur here linked to Kontur on the server by another computer's list of servers (Ivan joined there, through
/// `port`), with its members read once: this computer's list doesn't have the project's server.
fn linked_elsewhere(
  server: &TestServer,
  kontur: &SharedProject,
  (runtime, folder): (&Runtime, &Path),
  (port, repo): (&Forwarder, &Path),
) -> Result<(), Box<dyn Error>> {
  let workspace = Workspace::new(AppDb::open(&folder.join("elsewhere.db"))?);
  workspace.open(repo)?;
  let elsewhere = Servers::new(workspace, Vault::with(Arc::new(QaVault::default())));
  join(server, kontur, (&elsewhere, runtime), port)?;
  link(&elsewhere, runtime, repo, false)?;
  let open = ProjectServer::open(&elsewhere, repo)?.ok_or("linked")?;
  runtime.block_on(open.current_members())?;
  runtime.block_on(open.sync())?;
  Ok(())
}

fn offline_route(state: State, project: ProjectId, tasks: &Route) -> Route {
  match state {
    State::MakeLocalOffline => Route::Project(project, ProjectPage::Sharing { make_local: true }),
    State::TasksOffline => tasks.clone(),
    _ => Route::Project(project, ProjectPage::Sharing { make_local: false }),
  }
}

/// Ivan joins Kontur with a project code, through `port`, in `servers`.
fn join(
  server: &TestServer,
  kontur: &SharedProject,
  (servers, runtime): (&Servers, &Runtime),
  port: &Forwarder,
) -> Result<(), Box<dyn Error>> {
  let request = CodeRequest {
    code: server.project_code(kontur),
    username: Some("ivan".into()),
    display_name: Some("Ivan Zaida".into()),
    proof: Some(server.code_proof("ivan")),
  };
  let address = ServerAddress::parse(&port.url())?;
  runtime.block_on(servers.join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request))?;
  Ok(())
}

/// Links Kontur here to Kontur on the server, as the person's *Link and upload* (or *Link*) on its page would, with
/// `orchester.toml` when `write_discovery`.
fn link(servers: &Servers, runtime: &Runtime, repo: &Path, write_discovery: bool) -> Result<(), Box<dyn Error>> {
  let entry = servers.entries()?.remove(0);
  runtime.block_on(async {
    let kontur = servers.find_project(&entry, "kontur").await?;
    let plan = servers.link_plan(&entry, repo, kontur).await?;
    let request = LinkRequest {
      remote: None,
      key: IdempotencyKey::generate(),
      write_discovery,
    };
    servers.link_checkout(&entry, repo, &plan, request).await?;
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

/// A local task titled `title`, as another tab or an MCP client would add it.
pub fn add_local_task(repo: &Path, title: &str) -> Result<(), Box<dyn Error>> {
  let details = TaskDetails {
    title: Name::new(title)?,
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  ProjectDb::open(repo)?.create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())?;
  Ok(())
}

fn local_tasks(repo: &Path) -> Result<(), Box<dyn Error>> {
  let mut db = ProjectDb::open(repo)?;
  let details = LOCAL
    .iter()
    .map(|title| {
      Ok(TaskDetails {
        title: Name::new(title)?,
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
