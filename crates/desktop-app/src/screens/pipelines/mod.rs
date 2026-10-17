//! Pipelines library and editor (Pencil layers i4jt6V and vEDYY).
mod canvas;
mod command_picker;
mod dependencies;
mod editor;
mod graph_routes;
mod history;
mod inspector;
mod leaving;
mod library;
mod load_editor;
mod new_pipeline;
mod parts;
mod publication;
mod restore;
mod settings;
mod settings_nav;
mod state;
mod step_executor;
mod step_fields;
mod step_permissions;
mod summary;
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
/// The editor of pipeline `id` on the route's panel, of the draft or of the published version viewed.
pub fn editor(
  ctx: &mut Ctx,
  project: &ShownProject,
  id: ResourceId,
  (panel, version): (state::Panel, Option<VersionNumber>),
) -> Element {
  ctx.mount_keyed::<load_editor::LoadEditor>(
    &format!("pipeline-{id}"),
    load_editor::LoadProps {
      project: project.clone(),
      id,
      panel,
      version,
    },
  )
}
