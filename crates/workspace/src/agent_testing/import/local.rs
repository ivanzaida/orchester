//! A program of the checkout as a service: a package manager's script or `cargo run`, started without a shell in the
//! folder of its manifest, which it owns. Orchester gives every service a port of its own per run and sets `PORT` to
//! it, which many servers listen on; which port a program really listens on cannot be read from its manifest, so
//! that is reported for the person to check.

use std::collections::BTreeMap;

use orchester_core::{
  resource::{CommandArg, CommandProgram, VariableName},
  service::{OwnedPath, Readiness, RelativeFolder, Service, ServiceName, ServiceValue, ServiceVariables},
};

use super::Unmapped;
use crate::messages::Message;

/// The variable that carries the service's port.
const PORT_VARIABLE: &str = "PORT";

/// Service `name` running `program` with `arguments` in the checkout's folder `folder`, ready by `readiness`, with
/// what of the source's `key` Orchester could not read; `Err` says why it is not a service.
pub(super) fn service(
  (name, key): (ServiceName, &str),
  (program, arguments): (&str, &[&str]),
  folder: &str,
  readiness: Readiness,
) -> Result<(Service, Vec<Unmapped>), Message> {
  let invalid = |error: String| Message::new("services.import.skipped.invalid").with("reason", error);
  let program = CommandProgram::new(program).map_err(|error| invalid(error.to_string()))?;
  let arguments = arguments
    .iter()
    .map(|argument| CommandArg::new(argument))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| invalid(error.to_string()))?;
  let working_folder = RelativeFolder::new(folder).map_err(|error| invalid(error.to_string()))?;
  let port = VariableName::new(PORT_VARIABLE).expect("PORT is a variable name");
  let variables = ServiceVariables::new(BTreeMap::from([(port, ServiceValue::Plain("{{port}}".to_owned()))]))
    .expect("one variable is named once");
  let owned = if working_folder.is_root() {
    Vec::new()
  } else {
    vec![OwnedPath::new(working_folder.as_str()).map_err(|error| invalid(error.to_string()))?]
  };
  let service = Service::new(name, program, readiness)
    .with_arguments(arguments)
    .with_working_folder(working_folder)
    .with_variables(variables)
    .with_owned_paths(owned)
    .expect("one owned path is listed once");
  let port = Unmapped {
    key: key.to_owned(),
    reason: Message::new("services.import.unmapped.port").with("variable", PORT_VARIABLE.to_owned()),
  };
  Ok((service, vec![port]))
}
