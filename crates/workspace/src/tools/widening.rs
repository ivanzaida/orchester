//! Changes to commands and variables that need a person's decision, shared by the window and MCP.
use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  permission::widening::harmless_variable,
  resource::{CommandAction, CommandEnvironment, Tool, ToolAction},
};

use crate::messages::Message;

/// What `after` runs that `before` did not: another program, other arguments, exit codes that now pass,
/// or variables changed but harmless ones. Narrower exit codes, the description, the timeout and the
/// output limit change nothing a person must decide.
pub fn widenings(before: &CommandAction, after: &CommandAction) -> Vec<Message> {
  let mut reasons = Vec::new();
  if before.program() != after.program() {
    reasons.push(
      Message::new("appMcp.projectTools.widens.program")
        .with("program", after.program().as_str().to_owned())
        .with("before", before.program().as_str().to_owned()),
    );
  }
  if before.args() != after.args() {
    reasons.push(Message::new("appMcp.projectTools.widens.arguments"));
  }
  let passing: Vec<String> = after
    .exit_codes()
    .iter()
    .filter(|code| !before.exit_codes().contains(*code))
    .map(|code| code.to_string())
    .collect();
  if !passing.is_empty() {
    reasons.push(Message::new("appMcp.projectTools.widens.exitCodes").with("codes", passing.join(", ")));
  }
  reasons.extend(changed_variables(before.environment(), after.environment()));
  reasons
}

/// The variables `new` changes from `old` but harmless ones, each a reason a person decides it: a
/// removal included, since removing a person's proxy or certificate setting can widen as well.
pub fn changed_variables(old: &CommandEnvironment, new: &CommandEnvironment) -> Vec<Message> {
  let old: BTreeMap<String, &str> = old.iter().map(|(name, value)| (name.to_string(), value)).collect();
  let new: BTreeMap<String, &str> = new.iter().map(|(name, value)| (name.to_string(), value)).collect();
  old
    .keys()
    .chain(new.keys())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .filter(|name| old.get(*name) != new.get(*name) && !harmless_variable(name))
    .map(|name| Message::new("appMcp.widens.variable").with("name", name.clone()))
    .collect()
}

/// A new command tool or a command changed over the version being compared.
pub fn changes(before: Option<&Tool>, after: &Tool, first: &'static str) -> Vec<Message> {
  match (before.map(Tool::action), after.action()) {
    (Some(ToolAction::Command(before)), ToolAction::Command(after)) => widenings(before, after),
    _ => vec![Message::new(first)],
  }
}
