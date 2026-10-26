//! Pipelines (`Pipelines · redesign` `evQ4U`): the list, New pipeline, and a pipeline's page with its Steps,
//! Settings and Versions tabs.
mod actions;
mod column;
mod command_picker;
mod deliver;
mod delivers;
mod dependencies;
mod editor;
mod flow;
mod flow_card;
mod header;
mod leaving;
mod library;
mod library_row;
mod load_editor;
mod new_pipeline;
mod new_start;
mod parts;
mod publication;
mod restore;
mod settings;
mod sources;
mod state;
mod step_access;
mod step_column;
mod step_executor;
mod step_fields;
mod step_instructions;
mod step_permissions;
mod summary;
mod versions;
mod viewing;
mod work;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{id::ResourceId, resource::VersionNumber};

use crate::project::ShownProject;

fn work_error(error: orchester_workspace::pipelines::data::OpenError) -> orchester_workspace::messages::Message {
  orchester_workspace::pipelines::write::WriteError::from(error).message()
}

pub fn screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<library::Library>(&format!("pipelines-{}", project.open.project.id()), project.clone())
}
pub fn new_pipeline(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<new_pipeline::NewPipeline>(&format!("pipeline-new-{}", project.open.project.id()), project.clone())
}
/// The page of pipeline `id` as the route shows it, of the draft or of the published version viewed.
pub fn editor(
  ctx: &mut Ctx,
  project: &ShownProject,
  id: ResourceId,
  (view, version): (state::View, Option<VersionNumber>),
) -> Element {
  ctx.mount_keyed::<load_editor::LoadEditor>(
    &format!("pipeline-{id}"),
    load_editor::LoadProps {
      project: project.clone(),
      id,
      view,
      version,
    },
  )
}
