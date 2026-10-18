//! The names of the projects the app knows, for what the tooltip of a link to another project's place says
//! ([`super::place`]).

use std::{collections::HashMap, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background,
  messages::{self, Message},
  workspace::Workspace,
};

/// The names of the projects the app knows, by id; empty until they are read, or when they cannot be.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectNames(Arc<HashMap<ProjectId, Arc<str>>>);

impl lurq::app::component::DevtoolsInspectable for ProjectNames {}

impl ProjectNames {
  pub fn get(&self, project: ProjectId) -> Option<&str> {
    self.0.get(&project).map(|name| &**name)
  }

  /// The names of the projects `workspace` knows, read off the UI thread again whenever `revision` changes. Without
  /// them a link to another project's place names only the place, so a failed reading is logged and reads as none.
  pub fn read(ctx: &mut Ctx, workspace: Workspace, revision: u64) -> Self {
    let state = ctx
      .future((workspace, revision), |(workspace, _)| async move {
        let projects = background::run(move || workspace.projects()).await?.map_err(|error| {
          tracing::warn!(%error, "cannot read the projects' names for the chat's links");
          messages::store_error(&error)
        })?;
        let names = projects
          .into_iter()
          .map(|project| (project.id, Arc::from(project.name.as_str())))
          .collect();
        Ok::<_, Message>(Self(Arc::new(names)))
      })
      .state()
      .get();
    state.data.unwrap_or_default()
  }
}
