//! Instruction operations off the event loop, recorded in the session's activity.
use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
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

use super::fields::{Fields, Snapshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Write {
  Create(Name, Instruction),
  Save(InstructionEntry, Instruction),
  Publish(
    InstructionEntry,
    orchester_workspace::instructions::preview::PreviewChoice,
  ),
  Reload(orchester_core::id::ResourceId, Snapshot),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done {
  pub entry: InstructionEntry,
  pub published: bool,
  pub reload: Option<Snapshot>,
}
impl DevtoolsInspectable for Done {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Attempt(Option<Write>);
impl DevtoolsInspectable for Attempt {}
#[derive(Clone)]
pub struct Work {
  pub action: FutureAction<Write, Done, Message>,
  last: Signal<Attempt>,
  frozen: Signal<bool>,
}
impl Work {
  pub fn run(&self, write: Write) {
    if self.action.state().get_untracked().is_pending() {
      return;
    }
    self.frozen.set(matches!(write, Write::Create(..)));
    self.last.set(Attempt(Some(write.clone())));
    self.action.run(write);
  }
  pub fn retry(&self, fields: &Fields) -> Option<Write> {
    // Fields display parse errors; invalid current input disables a write retry.
    match self.last.get().0? {
      Write::Reload(id, _) => Some(Write::Reload(id, fields.snapshot())),
      Write::Save(expected, _) => fields.instruction().ok().map(|body| Write::Save(expected, body)),
      Write::Publish(expected, _) => fields
        .instruction()
        .ok()
        .filter(|body| *body == expected.body)
        .filter(|_| fields.blocked.get().0.is_empty())
        .map(|_| Write::Publish(expected, fields.preview_choice.get().0)),
      Write::Create(..) => fields
        .instruction()
        .ok()
        .and_then(|body| Name::new(&fields.name.get()).ok().map(|name| Write::Create(name, body))),
    }
  }
}

pub fn action(ctx: &mut Ctx, project: &OpenProject, fields: &Fields) -> Work {
  let project = project.clone();
  let services = crate::services::Services::of(ctx);
  let action = ctx.future_action(move |write: Write| {
    let (project, services) = (project.clone(), services.clone());
    async move {
      if let Write::Reload(id, requested) = write {
        services.hold.released().await;
        return background::run(move || {
          let entry = orchester_workspace::instructions::data::entry_in(&project.repo_root, id)
            .map_err(|error| orchester_workspace::messages::store_error(&error))?;
          Ok(Done {
            entry,
            published: false,
            reload: Some(requested),
          })
        })
        .await?;
      }
      record(&project, &services, write).await
    }
  });
  Work {
    action,
    last: ctx.signal(Attempt(None)),
    frozen: fields.frozen.clone(),
  }
}
async fn record(project: &OpenProject, services: &crate::services::Services, write: Write) -> Result<Done, Message> {
  let spec = match &write {
    Write::Create(name, _) => operations::instruction_create(project, name.as_str()),
    Write::Save(entry, _) => operations::instruction_draft(project, entry.id, &entry.name),
    Write::Publish(entry, _) => operations::instruction_publish(
      project,
      entry.id,
      &entry.name,
      entry.state.published().map_or(1, |v| v.get().saturating_add(1)),
    ),
    Write::Reload(..) => unreachable!("reload returns before recording a write"),
  };
  let activity = services.activity.start(spec);
  let published = matches!(write, Write::Publish(..));
  let root = project.repo_root.clone();
  let result = background::run(move || match write {
    Write::Create(name, body) => editor::create_in(&root, name, &body),
    Write::Save(entry, body) => editor::save_in(&root, &entry, body),
    Write::Publish(entry, choice) => editor::publish_for_in(&root, &entry, &choice),
    Write::Reload(..) => unreachable!("reload returns before a write"),
  })
  .await
  .and_then(|result| result);
  match &result {
    Ok(_) => activity.finish(),
    Err(error) => activity.fail(error.clone()),
  }
  result.map(|entry| {
    services.changes.record(project.project.id(), Outside::Instructions);
    Done {
      entry,
      published,
      reload: None,
    }
  })
}
