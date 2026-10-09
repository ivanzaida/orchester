//! Work behind the detected repository block, off the UI thread and reported to the status bar:
//! detecting the folder's repository (and, for an existing project, recording what the check
//! found), and choosing a `gh` program file.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  connection::{GitHubConnection, GitHubHost, GitHubLogin},
  id::ConnectionId,
  name::Name,
  repository::Detection,
  time::Timestamp,
};

use crate::{
  activity::operations,
  background,
  messages::Message,
  repository::{DetectTarget, detect, refresh_in},
  workspace::OpenProject,
};

pub type DetectAction = FutureAction<DetectTarget, Detected, Message>;
/// The dialog title; the chosen `gh` program, if one was chosen.
pub type ProgramAction = FutureAction<Arc<str>, Option<ChosenPath>, Message>;

/// A detection, as the screens' futures hold it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detected(pub Detection);

impl lurq::app::component::DevtoolsInspectable for Detected {}

/// A folder or program file chosen in a native dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChosenPath(pub PathBuf);

impl lurq::app::component::DevtoolsInspectable for ChosenPath {}

/// The account a project being set up will expect, if it expects one yet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Expected(pub Option<GitHubLogin>);

impl lurq::app::component::DevtoolsInspectable for Expected {}

/// The finished detection of `action`; `None` while it runs or before it ran.
pub fn finished(action: &DetectAction) -> Option<Detection> {
  let state = action.state().get();
  let pending = state.is_pending();
  state.data.filter(|_| !pending).map(|detected| detected.0)
}

/// The name a GitHub connection gets when setup or linking adds it. Only github.com can lack a
/// connection while detection asks `gh` about it, so that is the host it names.
pub fn connection_name(ctx: &Ctx) -> String {
  ctx
    .t_args("github.dialog.suggestedName", [("host", GitHubHost::GITHUB_COM)])
    .to_string()
}

/// The connection detection used, as a connection with id `id` that is not stored: the host,
/// the `gh` asked and its check. `None` when no `gh` answered.
pub fn detected_connection(ctx: &Ctx, detection: &Detection, id: ConnectionId) -> Option<GitHubConnection> {
  let found = detection.github()?;
  let (program, check) = (found.program()?, found.check()?);
  // The name comes from the locale and the program from detection, which found it on the PATH
  // or had it chosen as a file: both are valid, so a failure only leaves the panel closed.
  let name = Name::new(&connection_name(ctx)).ok()?;
  let mut connection = GitHubConnection::new(
    id,
    name,
    found.repo().host().clone(),
    program.to_path_buf(),
    Timestamp::now(),
  )
  .ok()?;
  connection.record_check(check.clone());
  Some(connection)
}

/// Detects a folder's repository. For an existing `project`, the check is recorded on its
/// connection (added for github.com when missing) and the link is refreshed when the
/// linked repository was read.
pub fn detect_action(ctx: &mut Ctx, project: Option<OpenProject>) -> DetectAction {
  let activity = crate::activity::of(ctx);
  let name = connection_name(ctx);
  ctx.future_action(move |target: DetectTarget| {
    let (activity, project, name) = (activity.clone(), project.clone(), name.clone());
    async move {
      let spec = operations::repository_detect(project.as_ref());
      let work = async move {
        background::run(move || {
          let detection = detect(&target);
          if project.is_some() {
            refresh_in(&target.repo_root, &detection, &name).map_err(|error| {
              tracing::error!(%error, "cannot record the repository check");
              error.message()
            })?;
          }
          Ok(Detected(detection))
        })
        .await?
      };
      activity.track(spec, work).await
    }
  })
}

/// The native file dialog for `gh` (gh not found).
pub fn program_action(ctx: &mut Ctx) -> ProgramAction {
  let activity = crate::activity::of(ctx);
  ctx.future_action(move |title: Arc<str>| {
    let activity = activity.clone();
    async move {
      // Listed while the dialog is open; closing it without a file leaves no event.
      let running = activity.start(operations::program_pick());
      let picked = background::run(move || rfd::FileDialog::new().set_title(&*title).pick_file()).await?;
      if picked.is_some() {
        running.finish();
      }
      Ok(picked.map(ChosenPath))
    }
  })
}
