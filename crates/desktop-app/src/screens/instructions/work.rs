//! Instruction operations off the event loop, recorded in the session's activity.
use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{name::Name, resource::Instruction};
use orchester_workspace::{
  activity::operations,
  background,
  instructions::{data::InstructionEntry, editor},
  messages::Message,
  tasks::outside::Outside,
  workspace::OpenProject,
};
#[derive(Clone)]
pub enum Write {
  Create(Name, Instruction),
  Save(InstructionEntry, Instruction),
  Publish(InstructionEntry),
  Reload(orchester_core::id::ResourceId),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done {
  pub entry: InstructionEntry,
  pub published: bool,
  pub reloaded: bool,
}
impl DevtoolsInspectable for Done {}
pub type Work = FutureAction<Write, Done, Message>;

pub fn action(ctx: &mut Ctx, project: &OpenProject) -> Work {
  let project = project.clone();
  let services = crate::services::Services::of(ctx);
  ctx.future_action(move |write: Write| {
    let (project, services) = (project.clone(), services.clone());
    async move {
      if let Write::Reload(id) = write {
        return background::run(move || {
          let entry = orchester_workspace::instructions::data::entry_in(&project.repo_root, id)
            .map_err(|error| orchester_workspace::messages::store_error(&error))?;
          Ok(Done {
            entry,
            published: false,
            reloaded: true,
          })
        })
        .await?;
      }
      let spec = match &write {
        Write::Create(name, _) => operations::instruction_create(&project, name.as_str()),
        Write::Save(entry, _) => operations::instruction_draft(&project, entry.id, &entry.name),
        Write::Publish(entry) => operations::instruction_publish(
          &project,
          entry.id,
          &entry.name,
          entry.state.published().map_or(1, |v| v.get().saturating_add(1)),
        ),
        Write::Reload(_) => unreachable!("reload returns before recording a write"),
      };
      let activity = services.activity.start(spec);
      let published = matches!(write, Write::Publish(_));
      let project_id = project.project.id();
      let result = background::run(move || match write {
        Write::Create(name, body) => editor::create_in(&project.repo_root, name, &body),
        Write::Save(entry, body) => editor::save_in(&project.repo_root, &entry, body),
        Write::Publish(entry) => editor::publish_in(&project.repo_root, &entry),
        Write::Reload(_) => unreachable!("reload returns before a write"),
      })
      .await
      .and_then(|result| result);
      match &result {
        Ok(_) => activity.finish(),
        Err(error) => activity.fail(error.clone()),
      }
      result.map(|entry| {
        services.changes.record(project_id, Outside::Instructions);
        Done {
          entry,
          published,
          reloaded: false,
        }
      })
    }
  })
}
