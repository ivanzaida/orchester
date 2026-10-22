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
  Create(Name, Instruction, Author),
  Save(InstructionEntry, Instruction, Author),
  Publish(InstructionEntry),
}
/// Who wrote the edits a save takes: an MCP client's synthetic input among them makes the draft automation's, marked
/// so on the page as one saved over MCP is (ORC-30).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Author {
  Person,
  Automation,
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
  fields: Fields,
}
impl Work {
  /// Starts `write` unless an operation is in flight. A save or a creation is automation's when an MCP client's input
  /// edited what it saves or asks for it now (see [`Fields::author`]).
  pub fn run(&self, write: Write) {
    let creating = matches!(write.change(), Some(Change::Create(..)));
    let write = match write {
      Write::Change(change) => Write::from(self.authored(*change)),
      reload @ Write::Reload(..) => reload,
    };
    if self.action.run_if_idle(write.clone()) {
      if matches!(write.change(), Some(Change::Save(..) | Change::Create(..))) {
        self.fields.saving();
      }
      self.fields.frozen.set(creating);
      self.last.set(Attempt(Some(write)));
    }
  }
  fn authored(&self, change: Change) -> Change {
    let author = |by: Author| match (by, self.fields.author()) {
      (Author::Person, Author::Person) => Author::Person,
      _ => Author::Automation,
    };
    match change {
      Change::Save(entry, body, by) => Change::Save(entry, body, author(by)),
      Change::Create(name, body, by) => Change::Create(name, body, author(by)),
      publish @ Change::Publish(..) => publish,
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
  /// The last write again, with the fields as they are now; a save keeps who wrote what it saved.
  pub fn retry(&self, fields: &Fields) -> Option<Write> {
    // Fields display parse errors; invalid current input disables a write retry.
    let last = match self.last.get().0? {
      Write::Reload(id, _) => return Some(Write::Reload(id, fields.snapshot())),
      Write::Change(change) => *change,
    };
    let change = match last {
      Change::Save(expected, _, by) => fields
        .instruction()
        .ok()
        .map(|body| Change::Save(expected, body, by)),
      Change::Publish(expected) => fields
        .instruction()
        .ok()
        .filter(|body| *body == expected.body)
        .filter(|_| fields.blocked().is_empty())
        .map(|_| Change::Publish(expected)),
      Change::Create(_, _, by) => fields
        .instruction()
        .ok()
        .and_then(|body| Name::new(&fields.name.get()).ok().map(|name| Change::Create(name, body, by))),
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
    fields: fields.clone(),
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
    Change::Create(name, ..) => operations::instruction_create(project, name.as_str()),
    Change::Save(entry, ..) => operations::instruction_draft(project, entry.id, &entry.name),
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
  let result = background::run(move || {
    let (saved, author) = match change {
      Change::Create(name, body, author) => (editor::create_in(&root, name, &body)?, author),
      Change::Save(entry, body, author) => (editor::save_in(&root, &entry, body)?, author),
      Change::Publish(entry) => return editor::publish_in(&root, &entry),
    };
    // A draft a client wrote through the window is marked as one saved over MCP is.
    if author == Author::Automation {
      editor::mark_automation_in(&root, &saved)?;
    }
    Ok(saved)
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
