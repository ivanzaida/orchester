//! Where a team action goes: the commands the team server keeps receipts of ([`command`]), or a definition's own
//! route ([`definition`]).

use orchester_core::id::OperationId;
use orchester_store::ProjectDb;

use super::{
  ActionError,
  action::{Action, ModuleAction, TaskAction},
  command::{self, TeamRequest},
  definition::{self, DefinitionEdit},
  ownership::TeamLink,
};

pub(super) enum TeamRoute {
  Command(TeamRequest),
  Definition(DefinitionEdit),
}

/// The route of `action`, the intent `operation`'s, on the team project `link`, with the server's ids of its targets.
pub(super) fn team_route(
  db: &ProjectDb,
  link: TeamLink,
  operation: OperationId,
  action: &Action,
) -> Result<TeamRoute, ActionError> {
  use TeamRoute::{Command, Definition};
  Ok(match action {
    Action::Task(TaskAction::Create { details, organization }) => Command(command::create_task(details, organization)?),
    Action::Task(TaskAction::Edit {
      task,
      read,
      base,
      edited,
    }) => Definition(definition::task(
      db,
      link,
      (*task, *read),
      definition::detail_changes(base, edited)?,
    )?),
    Action::Task(TaskAction::Organize {
      task,
      read,
      base,
      edited,
    }) => {
      let changes = definition::organization_changes(base, edited)?;
      Definition(definition::task(db, link, (*task, *read), changes)?)
    }
    Action::Task(TaskAction::Move { task, read, module }) => {
      Definition(definition::task(db, link, (*task, *read), definition::moved(*module))?)
    }
    Action::Module(ModuleAction::Create { details }) => Definition(definition::create_module(operation, details)),
    Action::Module(ModuleAction::Edit {
      module,
      read,
      base,
      edited,
    }) => Definition(definition::edit_module(operation, (*module, *read), base, edited)?),
    Action::Module(ModuleAction::AssignBranch { module, read, branch }) => {
      Command(command::module_branch(*module, *read, branch))
    }
    Action::Tag(action) => Definition(definition::tag(action)),
    Action::Policy(action) => Definition(definition::policy(*action)),
    Action::Claim(action) => Command(command::claim(db, link, action)?),
    Action::Assignment(action) => Command(command::assignment(db, link, *action)?),
    Action::Work(action) => Command(command::work(db, link, action)?),
    Action::Delivery(action) => Command(command::delivery(action)),
  })
}
