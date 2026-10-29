//! Changes to commands and variables that need a person's decision, shared by the window and MCP.
use orchester_core::{
  permission::widening::CommandWidening,
  resource::{CommandAction, Tool, ToolAction},
};

use crate::messages::Message;

/// What `after` runs that `before` did not ([`CommandAction::widenings_from`]), each a reason a person decides it.
/// The description changes nothing a person must decide either.
pub fn widenings(before: &CommandAction, after: &CommandAction) -> Vec<Message> {
  after
    .widenings_from(before)
    .into_iter()
    .map(|widening| match widening {
      CommandWidening::Program => Message::new("appMcp.projectTools.widens.program")
        .with("program", after.program().as_str().to_owned())
        .with("before", before.program().as_str().to_owned()),
      CommandWidening::Arguments => Message::new("appMcp.projectTools.widens.arguments"),
      CommandWidening::ExitCodes(codes) => {
        let codes: Vec<String> = codes.iter().map(i64::to_string).collect();
        Message::new("appMcp.projectTools.widens.exitCodes").with("codes", codes.join(", "))
      }
      CommandWidening::Variable(name) => variable(name),
    })
    .collect()
}

/// The reason a person decides a changed variable ([`orchester_core::permission::widening::changed_variables`]).
pub fn variable(name: String) -> Message {
  Message::new("appMcp.widens.variable").with("name", name)
}

/// A new command tool or a command changed over the version being compared.
pub fn changes(before: Option<&Tool>, after: &Tool, first: &'static str) -> Vec<Message> {
  match (before.map(Tool::action), after.action()) {
    (Some(ToolAction::Command(before)), ToolAction::Command(after)) => widenings(before, after),
    _ => vec![Message::new(first)],
  }
}
