//! Pipelines library and editor (Pencil layers i4jt6V and vEDYY).
mod canvas;
mod editor;
mod history;
mod inspector;
mod library;
mod new_pipeline;
mod parts;
mod settings;
mod state;
mod step_fields;
mod step_permissions;
mod work;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ResourceId;

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
pub fn editor(ctx: &mut Ctx, project: &ShownProject, id: ResourceId) -> Element {
  ctx.mount_keyed::<editor::LoadEditor>(
    &format!("pipeline-{id}"),
    editor::LoadProps {
      project: project.clone(),
      id,
    },
  )
}
