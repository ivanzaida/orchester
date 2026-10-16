//! The states with one registered project (N1 design 3): Kontur, in a git repository whose `origin` is the one Kontur
//! on the team server names, on a computer that joined Kontur team through an invite to Kontur, with an invite to
//! Billing waiting. They reach the All projects invite notice and Project settings › Sharing: *Link* (`project`), the
//! in-place *Link and upload* question (`link-confirm`, whose *Link* press the example makes as the person would,
//! [`open_sharing`]), and the linked page with *Leave* (`linked`, `L5o9Xy`).

use std::{error::Error, path::Path, process::Command};

use lurq::app::{App, Tree, events::MouseButton};
use orchester_app::{
  routes::{ProjectPage, Route},
  services::Services,
};
use orchester_core::{
  id::{IdempotencyKey, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_workspace::{
  project_setup::NewProject,
  sharing::{CloudJoined, project::LinkFirst},
  workspace::Workspace,
};
use tokio::runtime::Runtime;

use super::{State, backends::Backends};

/// Kontur's remote, the one the team server's Kontur names as its hint.
const KONTUR_REMOTE: &str = "https://github.com/acme/kontur.git";

/// Seeds `state` in `folder`; where the window opens.
pub fn seed(
  state: State,
  (backends, services, workspace): (&Backends, &Services, &Workspace),
  (runtime, folder): (&Runtime, &Path),
) -> Result<Route, Box<dyn Error>> {
  let repo = folder.join("repo-kontur");
  std::fs::create_dir_all(&repo)?;
  git(&repo, &["init", "-q"])?;
  git(&repo, &["remote", "add", "origin", KONTUR_REMOTE])?;
  let project = Project::new(ProjectId::generate(), ProjectKey::new("KON")?, Name::new("Kontur")?);
  workspace.create(NewProject {
    project: project.clone(),
    repo_root: repo.clone(),
  })?;
  let kontur = backends.invite_to("Kontur", &backends.kontur);
  let joined = runtime.block_on(async {
    let listed = services.cloud.invites().await?;
    let invite = listed
      .iter()
      .find(|listed| listed.id.to_string() == kontur)
      .ok_or("the invite to Kontur is listed")?;
    Ok::<_, Box<dyn Error>>(services.cloud.accept(invite).await?)
  })?;
  if !matches!(joined.joined, CloudJoined::Joined(_)) {
    return Err("the join was meant to finish".into());
  }
  backends.invite_to_billing();
  if state == State::Linked {
    let entry = services.servers.entries()?.remove(0);
    let request = LinkFirst {
      slug: "kontur".into(),
      remote: None,
      key: IdempotencyKey::generate(),
      write_discovery: false,
    };
    runtime.block_on(services.servers.link_first(&entry, &repo, request))?;
  }
  Ok(match state {
    State::Project => Route::AllProjects,
    _ => Route::Project(project.id(), ProjectPage::Settings),
  })
}

/// Opens Project settings › Sharing for `state`; for `link-confirm` also asks to link Kontur to `{server}/kontur`, as
/// the person would before *Link and upload*: that press is the example's own, not an MCP client's.
pub fn open_sharing(app: &mut App, tree: &mut Tree, (state, server): (State, &str)) -> Result<(), Box<dyn Error>> {
  until(app, tree, "settings-tab-sharing")?;
  press(app, tree, "settings-tab-sharing")?;
  if state != State::LinkConfirm {
    return Ok(());
  }
  until(app, tree, "sharing-address")?;
  tree
    .get_element_by_id_mut("sharing-address")
    .and_then(|element| element.as_text_input())
    .ok_or("sharing-address is a text input")?
    .set_value(format!("{server}/kontur"));
  tree.pass_headless(app);
  press(app, tree, "sharing-link-button")?;
  until(app, tree, "sharing-link-confirm")
}

/// Lays the window out, delivering what its futures finished, until `id` shows; an error after 20 seconds.
fn until(app: &mut App, tree: &mut Tree, id: &str) -> Result<(), Box<dyn Error>> {
  let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
  loop {
    tree.tick_futures();
    tree.pass_headless(app);
    if tree.get_element_by_id(id).is_some() {
      return Ok(());
    }
    if std::time::Instant::now() > deadline {
      return Err(format!("{id} did not show").into());
    }
    std::thread::sleep(std::time::Duration::from_millis(10));
  }
}

/// Presses the middle of `id` with the pointer.
fn press(app: &mut App, tree: &mut Tree, id: &str) -> Result<(), Box<dyn Error>> {
  let bounds = tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .ok_or_else(|| format!("{id} is laid out"))?;
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  tree.mouse_move(x, y);
  tree.mouse_down(x, y, MouseButton::Left);
  tree.mouse_up(x, y, MouseButton::Left);
  tree.pass_headless(app);
  Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Result<(), Box<dyn Error>> {
  let status = Command::new("git").args(args).current_dir(repo).status()?;
  if !status.success() {
    return Err(format!("git {args:?} failed").into());
  }
  Ok(())
}
