//! Instruction operations off the event loop, recorded in the session's activity.
use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_core::{id::ResourceId, name::Name, resource::Instruction};
use orchester_workspace::{
  activity::operations,
  background,
  instructions::{
    data::InstructionEntry,
    editor::{self, WriteError},
  },
  messages::Message,
  tasks::outside::Outside,
  workspace::OpenProject,
};

use super::fields::{Fields, Snapshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Write {
  Change(Box<Change>),
  /// Reads the instruction again, keeping the fields edited since `Snapshot`.
  Reload(ResourceId, Snapshot),
}
impl Write {
  pub fn change(&self) -> Option<&Change> {
    match self {
      Self::Change(change) => Some(change),
      Self::Reload(..) => None,
    }
  }
}
impl From<Change> for Write {
  fn from(change: Change) -> Self {
    Self::Change(Box::new(change))
  }
}
/// A write to the instruction, recorded in the session's activity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
  Create(Name, Instruction),
  Save(InstructionEntry, Instruction),
  Publish(InstructionEntry),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done {
  pub entry: InstructionEntry,
  pub published: bool,
  pub reload: Option<Snapshot>,
}
impl DevtoolsInspectable for Done {}
/// Why an operation did not happen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
  /// The saved instruction changed since it was read: read it again before a retry.
  Conflict(Message),
  Other(Message),
}
impl Failure {
  pub const fn message(&self) -> &Message {
    match self {
      Self::Conflict(message) | Self::Other(message) => message,
    }
  }
}
impl From<WriteError> for Failure {
  fn from(error: WriteError) -> Self {
    if error.needs_reload() {
      Self::Conflict(error.message())
    } else {
      Self::Other(error.message())
    }
  }
}
impl DevtoolsInspectable for Failure {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Attempt(Option<Write>);
impl DevtoolsInspectable for Attempt {}
#[derive(Clone)]
pub struct Work {
  pub action: FutureAction<Write, Done, Failure>,
  last: Signal<Attempt>,
  frozen: Signal<bool>,
}
impl Work {
  /// Starts `write` unless an operation is in flight.
  pub fn run(&self, write: Write) {
    let creating = matches!(write.change(), Some(Change::Create(..)));
    if self.action.run_if_idle(write.clone()) {
      self.frozen.set(creating);
      self.last.set(Attempt(Some(write)));
    }
  }
  pub fn publishing(&self) -> bool {
    self.action.state().get().is_pending() && self.last_publishes()
  }
  pub fn publication_failed(&self) -> bool {
    self.action.state().get().is_rejected() && self.last_publishes()
  }
  fn last_publishes(&self) -> bool {
    matches!(
      self.last.get().0.as_ref().and_then(Write::change),
      Some(Change::Publish(..))
    )
  }
  pub fn retry(&self, fields: &Fields) -> Option<Write> {
    // Fields display parse errors; invalid current input disables a write retry.
    let last = match self.last.get().0? {
      Write::Reload(id, _) => return Some(Write::Reload(id, fields.snapshot())),
      Write::Change(change) => *change,
    };
    let change = match last {
      Change::Save(expected, _) => fields.instruction().ok().map(|body| Change::Save(expected, body)),
      Change::Publish(expected) => fields
        .instruction()
        .ok()
        .filter(|body| *body == expected.body)
        .filter(|_| fields.blocked().is_empty())
        .map(|_| Change::Publish(expected)),
      Change::Create(..) => fields.instruction().ok().and_then(|body| {
        Name::new(&fields.name.get())
          .ok()
          .map(|name| Change::Create(name, body))
      }),
    };
    change.map(Write::from)
  }
}

pub fn action(ctx: &mut Ctx, project: &OpenProject, fields: &Fields) -> Work {
  let project = project.clone();
  let services = crate::services::Services::of(ctx);
  let action = ctx.future_action(move |write: Write| {
    let (project, services) = (project.clone(), services.clone());
    async move {
      match write {
        Write::Reload(id, requested) => reload(&project, &services, id, requested).await,
        Write::Change(change) => record(&project, &services, *change).await,
      }
    }
  });
  Work {
    action,
    last: ctx.signal(Attempt(None)),
    frozen: fields.frozen.clone(),
  }
}
async fn reload(
  project: &OpenProject,
  services: &crate::services::Services,
  id: ResourceId,
  requested: Snapshot,
) -> Result<Done, Failure> {
  services.hold.released().await;
  let root = project.repo_root.clone();
  let entry = background::run(move || orchester_workspace::instructions::data::entry_in(&root, id))
    .await
    .map_err(Failure::Other)?
    .map_err(|error| Failure::Other(orchester_workspace::messages::store_error(&error)))?;
  Ok(Done {
    entry,
    published: false,
    reload: Some(requested),
  })
}
async fn record(project: &OpenProject, services: &crate::services::Services, change: Change) -> Result<Done, Failure> {
  let spec = match &change {
    Change::Create(name, _) => operations::instruction_create(project, name.as_str()),
    Change::Save(entry, _) => operations::instruction_draft(project, entry.id, &entry.name),
    Change::Publish(entry) => operations::instruction_publish(
      project,
      entry.id,
      &entry.name,
      entry.state.published().map_or(1, |v| v.get().saturating_add(1)),
    ),
  };
  let activity = services.activity.start(spec);
  let published = matches!(change, Change::Publish(..));
  let root = project.repo_root.clone();
  let result = background::run(move || match change {
    Change::Create(name, body) => editor::create_in(&root, name, &body),
    Change::Save(entry, body) => editor::save_in(&root, &entry, body),
    Change::Publish(entry) => editor::publish_in(&root, &entry),
  })
  .await
  .map_err(Failure::Other)
  .and_then(|result| result.map_err(Failure::from));
  match &result {
    Ok(_) => activity.finish(),
    Err(error) => activity.fail(error.message().clone()),
  }
  result.map(|entry| {
    // Every screen that shows the instructions reads them again; Instructions itself reads them again only so.
    services.changes.record(project.project.id(), Outside::Instructions);
    Done {
      entry,
      published,
      reload: None,
    }
  })
}
