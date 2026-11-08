//! The QA app's team scenarios (`team-*` in the seed's registry): the stand-in team server *Acme* on loopback, sharing
//! Kontur and Billing, which answers while the window is open, with Kontur linked to the team's Kontur and joined by
//! code where the [`State`] says. The link comes first: a join marks as downloading only the checkouts already linked.
//! What a person presses is in [`drive`], and for `team-clone` in the QA app's `team_clone`. Fixture data for QA, never
//! shipped: an example is no part of the app's program.

pub mod drive;
mod member;
mod recorded_start;
mod state;
#[path = "../../sign_in_qa/window.rs"]
pub mod window;
mod work;

use std::{
  collections::HashMap,
  error::Error,
  path::{Path, PathBuf},
};

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
  test_support::team_server::{BundleFault, CODE, TasksAnswer, TeamServer, USERNAME},
};
use orchester_app::services::Services;
use orchester_core::{
  id::{ResourceId, ServerId},
  project::ProjectKey,
  resource::ResourceKind,
  sharing::{MemberRole, TaskRevision},
  store::{ResourceStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, ResourceSharing, SharingLink};
use orchester_workspace::sharing::{CodeRequest, Servers, Trust, project::FirstDownload, team_join::JoinOutcome};
pub use state::State;
use tokio::runtime::Handle;

/// The team's name, as the design gives it.
const TEAM: &str = "Acme";
/// The team's Kontur.
const SLUG: &str = "kontur";
/// The repository of the team's Kontur, as frame 08 names it (`YN6MO`), where the state says so; the seed gives
/// Kontur's checkout the same `origin`.
const REMOTE: &str = "github.com/acme/kontur";
/// How many tasks the team's Kontur has, and how many of them came down, as frame 08 counts them (`YN6MO`).
const TASKS: (usize, usize) = (212, 84);
/// The server's refusal, as the workspace words it.
const REFUSED: &str = "servers.errors.refused";
/// Acme's projects in `team-clone`, `(name, slug, remote hint)`: one per way a clone from *Link a folder* ends, in the
/// order the QA app's `team_clone` clones them, Kontur last, so the window stays on its page.
pub const CLONED: [(&str, &str, Option<&str>); 10] = [
  ("Atlas", "atlas", Some("github.com/acme/atlas")),
  ("Billing", "billing", Some("github.com/acme/billing")),
  ("Courier", "courier", Some("github.com/acme/courier")),
  ("Depot", "depot", Some("github.com/acme/depot")),
  ("Gateway", "gateway", Some("github.com/acme/gateway")),
  ("Harbor", "harbor", Some("github.com/acme/harbor")),
  ("Ingest", "ingest", Some("github.com/acme/ingest")),
  ("Juno", "juno", Some("github.com/acme/juno")),
  ("Ledger", "ledger", Some("github.com/acme/ledger")),
  ("Kontur", SLUG, Some(REMOTE)),
];

/// Sets `state` up for the session of `services`; the server, to hold while the window is open.
pub fn prepare(state: State, services: &Services, runtime: &Handle) -> Result<TeamServer, Box<dyn Error>> {
  let server = TeamServer::named(TEAM);
  let paged = matches!(state, State::DownloadProgress | State::DownloadStopped);
  server.share(&[("Kontur", SLUG, paged.then_some(REMOTE)), ("Billing", "billing", None)]);
  if paged {
    server.page_tasks(TASKS.0, TASKS.1);
  }
  match state {
    State::Join | State::WrongCode | State::Joined => {}
    State::ExpiredCode => server.expire_later_code(),
    State::Unreachable => server.set_down(true),
    State::Offer => join_linked(&server, services, runtime, TasksAnswer::Serve).map(drop)?,
    State::DownloadFailed | State::DownloadStopped => {
      join_linked(&server, services, runtime, TasksAnswer::Fail).map(drop)?;
    }
    State::Downloading | State::DownloadProgress => {
      let kontur = join_linked(&server, services, runtime, TasksAnswer::Hold)?;
      keep_downloading(services.servers.clone(), runtime, kontur);
    }
    State::SharedProjects => {
      server.share(&[
        ("Kontur", SLUG, Some("github.com/acme/kontur")),
        ("Billing", "billing", Some("github.com/acme/billing")),
      ]);
      server.own("billing");
      let address = add(&server, services, runtime)?.0;
      join(&address, services, runtime)?;
    }
    State::NoSharedProjects => {
      server.share(&[]);
      let address = add(&server, services, runtime)?.0;
      join(&address, services, runtime)?;
    }
    State::Clones => {
      server.share(&CLONED);
      let address = add(&server, services, runtime)?.0;
      join(&address, services, runtime)?;
    }
    State::Member
    | State::Admin
    | State::ServerAdmin
    | State::Outage
    | State::OutageRunning
    | State::OutageLater
    | State::MemberRunning
    | State::ModuleOutage
    | State::Claims
    | State::ModuleWork
    | State::AdminWork
    | State::VersionRefused
    | State::TeamRunning
    | State::EditConflict
    | State::NotReadyHere
    | State::StartFailed
    | State::StartRecovered
    | State::StartWaiting
    | State::StartRunnerRefused
    | State::StartPastFailure => {
      let ids = task_ids(services)?;
      let role = member::serve_as(&server, state, (&ids, &team_pipelines(services, state)?));
      let (kontur, id) = join_linked_as(&server, (services, runtime), (TasksAnswer::Serve, role))?;
      match state {
        State::StartFailed => server.authority_newer_bundles(),
        State::StartRecovered => {
          server.authority_bundle_fault(BundleFault::Unavailable);
          server.authority_bundle_fault_after(BundleFault::Newer, state::START_ANSWERS_AFTER);
        }
        State::StartWaiting => server.authority_bundle_fault(BundleFault::Unavailable),
        _ => {}
      }
      work::after_join(state, (&server, id), (services, runtime), (&kontur, &ids))?;
    }
  }
  Ok(server)
}

/// The session's Kontur's folder.
fn kontur(services: &Services) -> Result<PathBuf, Box<dyn Error>> {
  Ok(
    services
      .workspace
      .projects()?
      .into_iter()
      .find(|project| project.name.as_str() == "Kontur")
      .ok_or("the session has Kontur")?
      .repo_root,
  )
}

/// The ids of Kontur's tasks on this computer, by their number.
fn task_ids(services: &Services) -> Result<HashMap<u32, String>, Box<dyn Error>> {
  let tasks = ProjectDb::open(&kontur(services)?)?.tasks()?;
  Ok(
    tasks
      .iter()
      .map(|task| (task.key().number().get(), task.id().to_string()))
      .collect(),
  )
}

/// Kontur's Feature delivery and Quick fix as the team's pipelines, as Share leaves them (row 80): the tasks on the
/// server name them by these ids. A Kontur without one, as the headless tests' is, names a team pipeline it lacks.
/// `team-not-ready` names the team's own Team delivery and Team quick fix instead.
fn team_pipelines(services: &Services, state: State) -> Result<member::TeamPipelines, Box<dyn Error>> {
  let (feature, quick) = match state {
    State::NotReadyHere => ("team-delivery", "team-quick-fix"),
    // Every task is on Team quick fix, which is ready here: only the bundle fails.
    State::StartFailed
    | State::StartRecovered
    | State::StartWaiting
    | State::StartRunnerRefused
    | State::StartPastFailure => ("team-quick-fix", "team-quick-fix"),
    _ => ("feature-delivery", "quick-fix"),
  };
  let mut db = ProjectDb::open(&kontur(services)?)?;
  let mut id_of = |slug: &str| -> Result<String, Box<dyn Error>> {
    let found = db
      .resources(ResourceKind::Pipeline)?
      .into_iter()
      .find(|resource| resource.slug().as_str() == slug);
    let Some(pipeline) = found.map(|resource| resource.id()) else {
      return Ok(ResourceId::generate().to_string());
    };
    db.set_resource_sharing(pipeline, Some(ResourceSharing::Shared(TaskRevision::FIRST)))?;
    Ok(pipeline.to_string())
  };
  Ok(member::TeamPipelines {
    feature_delivery: id_of(feature)?,
    quick_fix: id_of(quick)?,
  })
}

/// Adds `server` to the list; its address and id.
fn add(
  server: &TeamServer,
  services: &Services,
  runtime: &Handle,
) -> Result<(ServerAddress, ServerId), Box<dyn Error>> {
  let address = ServerAddress::parse(&server.url())?;
  let id = runtime.block_on(services.servers.add(&address, TEAM.to_owned(), PlainHttp::NotAllowed))?;
  Ok((address, id))
}

/// Joins the server at `address` with the code.
fn join(address: &ServerAddress, services: &Services, runtime: &Handle) -> Result<(), Box<dyn Error>> {
  let request = CodeRequest {
    code: Credential::new(CODE.to_owned()),
    username: Some(USERNAME.to_owned()),
    display_name: None,
    proof: None,
  };
  let outcome = runtime.block_on(services.team_join.join_by_code(
    address,
    Trust::System(PlainHttp::NotAllowed),
    request,
  ))?;
  match outcome {
    JoinOutcome::Joined(_) => Ok(()),
    other => Err(format!("the stand-in team did not take its code: {other:?}").into()),
  }
}

/// Adds `server` to the list, links the session's Kontur to the team's as a member and joins with the code, its tasks
/// answered as `tasks`; Kontur's folder.
fn join_linked(
  server: &TeamServer,
  services: &Services,
  runtime: &Handle,
  tasks: TasksAnswer,
) -> Result<PathBuf, Box<dyn Error>> {
  join_linked_as(server, (services, runtime), (tasks, Some(MemberRole::Member))).map(|(kontur, _)| kontur)
}

/// [`join_linked`], with the person's role in Kontur `role` (none for the server's admin); the server's id too.
fn join_linked_as(
  server: &TeamServer,
  (services, runtime): (&Services, &Handle),
  (tasks, role): (TasksAnswer, Option<MemberRole>),
) -> Result<(PathBuf, ServerId), Box<dyn Error>> {
  server.answer_tasks(tasks);
  let kontur = kontur(services)?;
  let (address, id) = add(server, services, runtime)?;
  link(&kontur, server, (id, role))?;
  join(&address, services, runtime)?;
  Ok((kontur, id))
}

/// Links the project at `repo` to the team's Kontur on `server` with `id`, the person's role there `role`, with nothing
/// of it cached yet.
fn link(repo: &Path, server: &TeamServer, (id, role): (ServerId, Option<MemberRole>)) -> Result<(), Box<dyn Error>> {
  let link = SharingLink {
    server: id,
    project: server.project(SLUG).ok_or("the team shares Kontur")?,
    key: ProjectKey::new("KON")?,
    role,
    joined_at: Timestamp::now(),
    cursor: 0,
    last_synced_at: Timestamp::now(),
    removed_at: None,
    address: Some(format!("{}/{SLUG}", server.url())),
  };
  ProjectDb::open(repo)?.start_sharing(&link, &[], (&[], &Default::default()))?;
  Ok(())
}

/// Keeps the download of the checkout at `root` under way: a held answer ends in the server's refusal before the
/// client's timeout, and then the checkout is asked for again. Any other failure stays on screen, so this cannot spin
/// on a server that went away.
fn keep_downloading(servers: Servers, runtime: &Handle, root: PathBuf) {
  let mut changes = servers.subscribe_shared();
  runtime.spawn(async move {
    loop {
      changes.borrow_and_update();
      if let Some(FirstDownload::Failed { reason, .. }) = servers.download_state(&root)
        && reason.key == REFUSED
      {
        servers.retry_download(&root);
      }
      if changes.changed().await.is_err() {
        return;
      }
    }
  });
}
