//! Saves off the event loop, tracked in the shared activity log.
use std::path::PathBuf;

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::id::ResourceId;
use orchester_workspace::{
  activity::operations,
  background,
  connections::mcp::Allowed,
  messages::Message,
  tasks::outside::{Outside, OutsideCounts},
  tools::{
    data::ToolChange,
    editor::{self, Edit},
  },
  workspace::OpenProject,
};

use crate::services::Services;
#[derive(Clone)]
pub(super) struct Start {
  pub open: OpenProject,
  pub edit: Edit,
  pub allowed: Allowed,
}
/// The tool saved, and the project's change counts right after the save recorded its change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Saved(pub ResourceId, pub OutsideCounts);
impl DevtoolsInspectable for Saved {}
pub(super) type Work = FutureAction<Start, Saved, Message>;
pub(super) fn action(ctx: &mut Ctx) -> Work {
  let services = Services::of(ctx);
  let (activity, changes) = (services.activity, services.changes);
  ctx.future_action(move |start: Start| {
    let (activity, changes) = (activity.clone(), changes.clone());
    async move {
      let (name, change, version) = operation(&start.edit);
      let spec = operations::tool_change(&start.open, &name, &change, version);
      let root: PathBuf = start.open.repo_root.clone();
      let target = root.clone();
      let project = start.open.project.id();
      activity
        .track_to(
          spec,
          async move {
            background::run(move || {
              let saved = editor::apply_in(&root, start.edit, &start.allowed)?;
              // Right after the write, whether or not the editor is still there to take the save.
              Ok(Saved(saved, changes.record(project, Outside::Tools)))
            })
            .await?
          },
          move |saved| Some(operations::tool(&target, saved.0)),
        )
        .await
    }
  })
}
fn operation(edit: &Edit) -> (String, ToolChange, u32) {
  match edit {
    Edit::Save { name, tool, before } => match before {
      Some(e) => (e.name.to_string(), ToolChange::SaveDraft(e.id, (**tool).clone()), 0),
      None => (
        name.as_str().to_owned(),
        ToolChange::Create(name.clone(), (**tool).clone()),
        0,
      ),
    },
    Edit::Publish(e) => (
      e.name.to_string(),
      ToolChange::Publish(e.id),
      editor::next_version(Some(e)).map_or(0, |v| v.get()),
    ),
    Edit::KeepDraft(e) => (e.name.to_string(), ToolChange::KeepDraft(e.id), 0),
    Edit::StartLatest(e) => (e.name.to_string(), ToolChange::SaveDraft(e.id, e.tool.clone()), 0),
  }
}
