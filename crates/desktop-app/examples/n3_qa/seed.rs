//! What each N3 state starts from: a team server with Alice Moreau (its admin), Oleh Petrenko and Maria Lind; KTR
//! (Kontur, set up for `github.com/acme/kontur`) with four of Alice's tasks, two of them like Kontur's here; Billing,
//! which Ivan is a member of, and Atlas, which he isn't. This computer joined it as Ivan Zaida through a port the seed
//! can take down. Kontur here (KON) is a git checkout of that repository with six tasks, KON-2's run under way; the
//! states after the join leave KON-1 and KON-5 Not shared. The landing's states have other checkouts: `kontur-spike`, a
//! fork, and Billing, linked to Billing there. Fixture data for QA, never shipped.

use std::{
  error::Error,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_api::{
  SharedProject,
  client::{PlainHttp, ServerAddress},
};
use orchester_app::{
  routes::{JoinTarget, OrchesterView, ProjectPage, ProjectSettingsTab, Route, TaskList},
  services::Services,
};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, ServerId},
  name::Name,
  project::{Project, ProjectKey, TaskKey},
  resource::ResourceKind,
  store::{ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  project_setup::NewProject,
  sharing::{
    CodeRequest, Servers, Trust,
    project::{ProjectServer, first_link::LinkRequest},
  },
  workspace::Workspace,
};
use serde_json::json;
use tokio::runtime::Runtime;

use super::{State, forwarder::Forwarder, run_fixture, server_support::TestServer};

const KONTUR_REMOTE: &str = "https://github.com/acme/kontur.git";
const FORK_REMOTE: &str = "https://github.com/oleh/kontur.git";
const BILLING_REMOTE: &str = "https://github.com/acme/billing.git";

/// Alice's tasks on the server, KTR-1 … KTR-4.
const SERVER: [&str; 4] = [
  "Typed errors per module",
  "Show the signed-in account",
  "Export invoices to PDF",
  "Retry failed exports every night",
];

/// What the window needs while it runs: the servers, which stop when this is dropped, where it opens, and what the
/// example's own presses need.
pub struct Seeded {
  pub route: Route,
  /// Atlas's address through the port: a project Ivan isn't a member of.
  pub atlas: String,
  /// What the folder picker hands over.
  pub picked: PathBuf,
  pub port: Forwarder,
  _server: TestServer,
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
  let kontur = server.create_named_project(("Kontur", "KTR"), "kontur", Some(KONTUR_REMOTE));
  let contents: Vec<_> = SERVER.iter().map(|title| json!({ "title": title })).collect();
  server.add_tasks(&kontur, &contents);
  let billing = server.create_named_project(("Billing", "BIL"), "billing", Some(BILLING_REMOTE));
  server.create_named_project(("Atlas", "ATL"), "atlas", None);
  let port = Forwarder::start(server.address);
  join(&server, &kontur, (&services.servers, runtime), &port)?;
  server.add_member(&billing, "ivan");
  let entry = services.servers.entries()?.remove(0);
  let target = JoinTarget::new(entry.stored.id, "kontur").ok_or("a target")?;
  let checkouts = Checkouts { workspace, folder };
  let (route, picked) = match state {
    State::Landing
    | State::LandingNone
    | State::LandingAdded
    | State::LandingNotGit
    | State::LandingLinked
    | State::LandingDirect
    | State::Later => landing(state, (&checkouts, &target), (&services.servers, runtime))?,
    _ => (
      kontur_route(state, (&checkouts, &target), (services, runtime))?,
      folder.to_owned(),
    ),
  };
  Ok(Seeded {
    route,
    atlas: format!("{}/atlas", port.url()),
    picked,
    port,
    _server: server,
  })
}

/// Where the window opens for a state of Kontur here, joined or not as it needs.
fn kontur_route(
  state: State,
  (checkouts, target): (&Checkouts<'_>, &JoinTarget),
  (services, runtime): (&Services, &Runtime),
) -> Result<Route, Box<dyn Error>> {
  let (project, repo) = checkouts.kontur(true)?;
  let at = |page| Route::Project(project, page);
  let tasks = |list: TaskList| at(ProjectPage::Tasks(list));
  match state {
    State::JoinReview | State::JoinUnticked | State::JoinFailed | State::JoinPersonOnly => {
      return Ok(at(ProjectPage::Join(target.clone())));
    }
    State::NotMember => return Ok(at(ProjectPage::Settings(ProjectSettingsTab::Sharing))),
    _ => {}
  }
  joined(&services.servers, runtime, (&repo, checkouts.workspace, project))?;
  if state == State::TasksSharedLater {
    let open = ProjectServer::open(&services.servers, &repo)?.ok_or("joined")?;
    let kept = ProjectDb::open(&repo)?.not_shared_tasks()?;
    let task = kept
      .iter()
      .find(|task| task.key().to_string() == "KON-5")
      .ok_or("KON-5")?;
    runtime.block_on(open.share_kept(task.id(), IdempotencyKey::generate()))?;
  }
  Ok(match state {
    State::Joined => at(ProjectPage::Settings(ProjectSettingsTab::Sharing)),
    State::TasksNotShared => tasks(TaskList::default().with_not_shared(true)),
    State::TasksFormerKey => tasks(TaskList::default().with_search("KON-3")),
    State::TasksOnlyNotShared => tasks(TaskList::default().with_search("KON-5")),
    State::TasksNotSharedSearch => tasks(TaskList::default().with_search("invoices").with_not_shared(true)),
    State::TaskNotShared | State::TaskShare => at(ProjectPage::Task("KON-1".parse()?, Default::default())),
    State::TaskJoined => at(ProjectPage::Task(was(&repo, "KON-3")?, Default::default())),
    _ => tasks(TaskList::default()),
  })
}

/// The key of the task that was `former` before the join.
fn was(repo: &Path, former: &str) -> Result<TaskKey, Box<dyn Error>> {
  let db = ProjectDb::open(repo)?;
  let id = db
    .former_keys()?
    .into_iter()
    .find(|(_, key)| key == former)
    .map(|(id, _)| id)
    .ok_or("the former key")?;
  Ok(db.task(id)?.ok_or("the task")?.key().clone())
}

/// The landing's checkouts for `state`, where the window opens, and what the folder picker hands over.
fn landing(
  state: State,
  (checkouts, target): (&Checkouts<'_>, &JoinTarget),
  (servers, runtime): (&Servers, &Runtime),
) -> Result<(Route, PathBuf), Box<dyn Error>> {
  let invite = Route::Invite(target.clone());
  let folder = checkouts.folder;
  Ok(match state {
    State::Landing => {
      checkouts.kontur(true)?;
      checkouts.spike()?;
      (invite, folder.to_owned())
    }
    State::LandingDirect => {
      checkouts.kontur(true)?;
      (invite, folder.to_owned())
    }
    State::LandingNone | State::Later => {
      checkouts.billing(servers, runtime)?;
      let route = if state == State::Later {
        Route::Orchester(OrchesterView::server(target.server))
      } else {
        invite
      };
      (route, folder.to_owned())
    }
    State::LandingAdded => {
      checkouts.spike()?;
      let clone = folder.join("work").join("kontur");
      git_checkout(&clone, Some(KONTUR_REMOTE))?;
      (invite, clone)
    }
    State::LandingNotGit => {
      checkouts.spike()?;
      let archive = folder.join("Downloads").join("kontur-main");
      std::fs::create_dir_all(&archive)?;
      (invite, archive)
    }
    State::LandingLinked => {
      checkouts.spike()?;
      let billing = checkouts.billing(servers, runtime)?;
      (invite, billing)
    }
    _ => (invite, folder.to_owned()),
  })
}

/// The local projects a state creates in `folder`.
struct Checkouts<'a> {
  workspace: &'a Workspace,
  folder: &'a Path,
}

impl Checkouts<'_> {
  /// Kontur (KON), a checkout of the project's repository; with its six tasks, KON-2's run under way, when `tasks`.
  fn kontur(&self, tasks: bool) -> Result<(ProjectId, PathBuf), Box<dyn Error>> {
    let repo = self.folder.join("projects").join("kontur");
    let project = self.create(&repo, ("Kontur", "KON"), Some(KONTUR_REMOTE))?;
    if tasks {
      kontur_tasks(&repo)?;
    }
    Ok((project, repo))
  }

  /// `kontur-spike`, a checkout of a fork, with no tasks.
  fn spike(&self) -> Result<ProjectId, Box<dyn Error>> {
    let repo = self.folder.join("scratch").join("kontur-spike");
    self.create(&repo, ("kontur-spike", "KS"), Some(FORK_REMOTE))
  }

  /// Billing (BIL), linked to Billing on the server; its folder.
  fn billing(&self, servers: &Servers, runtime: &Runtime) -> Result<PathBuf, Box<dyn Error>> {
    let repo = self.folder.join("projects").join("billing");
    self.create(&repo, ("Billing", "BIL"), Some(BILLING_REMOTE))?;
    let entry = servers.entries()?.remove(0);
    runtime.block_on(async {
      let billing = servers.find_project(&entry, "billing").await?;
      let plan = servers.link_plan(&entry, &repo, billing).await?;
      let request = LinkRequest {
        remote: None,
        key: IdempotencyKey::generate(),
        write_discovery: false,
      };
      servers.link_checkout(&entry, &repo, &plan, request).await?;
      Ok::<_, Box<dyn Error>>(())
    })?;
    Ok(repo)
  }

  fn create(&self, repo: &Path, (name, key): (&str, &str), origin: Option<&str>) -> Result<ProjectId, Box<dyn Error>> {
    git_checkout(repo, origin)?;
    let project = Project::new(ProjectId::generate(), ProjectKey::new(key)?, Name::new(name)?);
    self.workspace.create(NewProject {
      project: project.clone(),
      repo_root: repo.to_owned(),
    })?;
    Ok(project.id())
  }
}

/// Kontur's six tasks: KON-2 complete on a published pipeline with its run under way.
fn kontur_tasks(repo: &Path) -> Result<(), Box<dyn Error>> {
  let mut db = ProjectDb::open(repo)?;
  create(&mut db, &["Export invoices as PDF"])?;
  run_fixture::auto_run(&mut db);
  let pipeline = db.resources(ResourceKind::Pipeline)?.remove(0).id();
  let task = db.tasks()?.remove(1);
  let complete = TaskDetails {
    title: Name::new("Add retry to webhook sender")?,
    brief: "Retry a failed delivery three times with backoff.".into(),
    acceptance_criteria: vec!["A delivery that fails twice succeeds on the third try.".into()],
    pipeline: Some(pipeline),
    ..task.details().clone()
  };
  db.update_task(task.id(), task.revision(), complete)?
    .saved()
    .ok_or("KON-2 was saved as read")?;
  create(
    &mut db,
    &[
      "Show delivery status on the endpoint page",
      "Paginate the webhook log",
      "Retry failed exports nightly",
      "Rate-limit messages from GitHub",
    ],
  )
}

fn create(db: &mut ProjectDb, titles: &[&str]) -> Result<(), Box<dyn Error>> {
  let details = titles
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

/// Kontur here joins KTR with KON-1 and KON-5 unticked, as the review's *Join and upload 4* would; Sharing says what
/// it did until dismissed, as after that press.
fn joined(
  servers: &Servers,
  runtime: &Runtime,
  (repo, workspace, project): (&Path, &Workspace, ProjectId),
) -> Result<(), Box<dyn Error>> {
  let entry = servers.entries()?.remove(0);
  let review = runtime.block_on(servers.join_review_at(&entry, repo, "kontur"))?;
  let ticked = review
    .tasks
    .iter()
    .filter(|task| !["KON-1", "KON-5"].contains(&task.task.key().to_string().as_str()))
    .map(|task| task.task.id())
    .collect();
  runtime.block_on(servers.join(&entry, repo, &review, (ticked, IdempotencyKey::generate())))?;
  workspace.set_ui_state(&format!("orchester-app.sharing.joined.{project}"), r#"{"dropped":[]}"#)?;
  Ok(())
}

/// Ivan joins Kontur with a project code, through `port`, in `servers`.
fn join(
  server: &TestServer,
  kontur: &SharedProject,
  (servers, runtime): (&Servers, &Runtime),
  port: &Forwarder,
) -> Result<ServerId, Box<dyn Error>> {
  let request = CodeRequest {
    code: server.project_code(kontur),
    username: Some("ivan".into()),
    display_name: Some("Ivan Zaida".into()),
    proof: Some(server.code_proof("ivan")),
  };
  let address = ServerAddress::parse(&port.url())?;
  Ok(runtime.block_on(servers.join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request))?)
}

/// A git checkout in `repo` whose `origin` is `origin`.
fn git_checkout(repo: &Path, origin: Option<&str>) -> Result<(), Box<dyn Error>> {
  std::fs::create_dir_all(repo)?;
  git(repo, &["init", "-q"])?;
  if let Some(origin) = origin {
    git(repo, &["remote", "add", "origin", origin])?;
  }
  Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Result<(), Box<dyn Error>> {
  let status = Command::new("git").args(args).current_dir(repo).status()?;
  if !status.success() {
    return Err(format!("git {args:?} failed").into());
  }
  Ok(())
}
