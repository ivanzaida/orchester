//! Changes Project settings makes, each off the UI thread and reported to the status bar:
//! *Use signed-in account*, linking the detected repository, and pointing the GitHub connection
//! at a chosen `gh` (gh not found).

use std::path::PathBuf;

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{connection::GitHubLogin, id::ConnectionId, repository::Detection};

use crate::{
  activity::{Activity, OperationSpec, operations},
  background,
  connections::github::{choose_program_in, expect_active_account_in},
  messages::Message,
  repository::link_in,
  screens::github::actions::connection_name,
  workspace::OpenProject,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsWork {
  /// Make the project expect the account `gh` of the connection acts as now.
  UseAccount { connection: ConnectionId, name: String },
  /// Link the detected repository, expecting `expected` or else the account `gh` acts as.
  Link {
    detection: Box<Detection>,
    expected: Option<GitHubLogin>,
  },
  /// Point the connection at a chosen program and check it.
  Program {
    connection: ConnectionId,
    name: String,
    program: PathBuf,
  },
}

impl lurq::app::component::DevtoolsInspectable for SettingsWork {}

/// Work that finished; `detect_again` when what detection found has changed with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Done {
  pub detect_again: bool,
}

impl lurq::app::component::DevtoolsInspectable for Done {}

pub type WorkAction = FutureAction<SettingsWork, Done, Message>;

pub fn work_action(ctx: &mut Ctx, open: OpenProject) -> WorkAction {
  let activity = Activity::of(ctx);
  let name = connection_name(ctx);
  ctx.future_action(move |work: SettingsWork| {
    let (activity, open, name) = (activity.clone(), open.clone(), name.clone());
    async move {
      let spec = spec(&open, &work);
      let run = async move {
        let detect_again = matches!(work, SettingsWork::Program { .. });
        background::run(move || run(&open, work, &name)).await??;
        Ok(Done { detect_again })
      };
      activity.track(spec, run).await
    }
  })
}

fn spec(open: &OpenProject, work: &SettingsWork) -> OperationSpec {
  match work {
    SettingsWork::UseAccount { name, .. } => operations::expected_account(open, name),
    SettingsWork::Link { detection, .. } => {
      let repository = detection
        .github()
        .map(|found| format!("{}/{}", found.repo().owner(), found.repo().name()))
        .unwrap_or_default();
      operations::repository_link(open.project.name().as_str(), &open.repo_root, &repository)
    }
    SettingsWork::Program { name, .. } => operations::connection_check(open, name),
  }
}

fn run(open: &OpenProject, work: SettingsWork, connection_name: &str) -> Result<(), Message> {
  let repo_root = &open.repo_root;
  match work {
    SettingsWork::UseAccount { connection, .. } => expect_active_account_in(repo_root, connection).map_err(|error| {
      tracing::error!(%error, "cannot record the account gh uses as the expected one");
      error.message()
    }),
    SettingsWork::Link { detection, expected } => link_in(repo_root, &detection, expected.as_ref(), connection_name)
      .map(|_| ())
      .map_err(|error| {
        tracing::error!(%error, "cannot link the detected repository");
        error.message()
      }),
    SettingsWork::Program {
      connection, program, ..
    } => choose_program_in(repo_root, connection, program).map_err(|error| {
      tracing::error!(%error, "cannot point the connection at the chosen program");
      error.message()
    }),
  }
}
