//! The QA app's team scenarios (`team-*` in the seed's registry): the stand-in team server *Acme* on loopback, sharing
//! Kontur and Billing, which answers while the window is open, with Kontur linked to the team's Kontur and joined by
//! code where the [`State`] says. The link comes first: a join marks as downloading only the checkouts already linked.
//! What a person presses is in [`drive`]. Fixture data for QA, never shipped: an example is no part of the app's
//! program.

pub mod drive;
mod state;

use std::{
  error::Error,
  path::{Path, PathBuf},
};

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
  test_support::team_server::{CODE, TasksAnswer, TeamServer, USERNAME},
};
use orchester_app::services::Services;
use orchester_core::{id::ServerId, project::ProjectKey, sharing::MemberRole, time::Timestamp};
use orchester_store::{ProjectDb, SharingLink};
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
  }
  Ok(server)
}

/// Adds `server` to the list, links the session's Kontur to the team's and joins with the code, its tasks answered as
/// `tasks`; Kontur's folder.
fn join_linked(
  server: &TeamServer,
  services: &Services,
  runtime: &Handle,
  tasks: TasksAnswer,
) -> Result<PathBuf, Box<dyn Error>> {
  server.answer_tasks(tasks);
  let kontur = services
    .workspace
    .projects()?
    .into_iter()
    .find(|project| project.name.as_str() == "Kontur")
    .ok_or("the session has Kontur")?
    .repo_root;
  let address = ServerAddress::parse(&server.url())?;
  let id = runtime.block_on(services.servers.add(&address, TEAM.to_owned(), PlainHttp::NotAllowed))?;
  link(&kontur, server, id)?;
  let request = CodeRequest {
    code: Credential::new(CODE.to_owned()),
    username: Some(USERNAME.to_owned()),
    display_name: None,
    proof: None,
  };
  let outcome = runtime.block_on(services.team_join.join_by_code(
    &address,
    Trust::System(PlainHttp::NotAllowed),
    request,
  ))?;
  match outcome {
    JoinOutcome::Joined(_) => Ok(kontur),
    other => Err(format!("the stand-in team did not take its code: {other:?}").into()),
  }
}

/// Links the project at `repo` to the team's Kontur on `server`, with nothing of it cached yet.
fn link(repo: &Path, server: &TeamServer, id: ServerId) -> Result<(), Box<dyn Error>> {
  let link = SharingLink {
    server: id,
    project: server.project(SLUG).ok_or("the team shares Kontur")?,
    key: ProjectKey::new("KON")?,
    role: Some(MemberRole::Member),
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
